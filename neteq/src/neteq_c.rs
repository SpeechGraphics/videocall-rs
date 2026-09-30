use std::cmp::min;
use std::ffi::c_void;
use std::sync::Mutex;

use crate::codec::OpusDecoder;
use crate::neteq::{NetEq, NetEqConfig};
use crate::packet::{AudioPacket, RtpHeader};

// FFI-safe subset of NetEqConfig's scalar fields.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NetEqConfigFfi {
    pub sample_rate: u32,
    pub channels: u8,
    pub max_packets_in_buffer: usize,
    pub max_delay_ms: u32,
    pub min_delay_ms: u32,
    pub additional_delay_ms: u32,
}

struct NetEqHandle {
    neteq: Mutex<NetEq>,
    sample_rate: u32,
    channels: u8,
}

#[no_mangle]
pub extern "C" fn neteq_create(config: NetEqConfigFfi) -> *mut c_void {
    let neteq_config = NetEqConfig {
        sample_rate: config.sample_rate,
        channels: config.channels,
        max_packets_in_buffer: config.max_packets_in_buffer,
        max_delay_ms: config.max_delay_ms,
        min_delay_ms: config.min_delay_ms,
        additional_delay_ms: config.additional_delay_ms,
        ..Default::default()
    };
    let neteq = match NetEq::new(neteq_config) {
        Ok(n) => n,
        Err(e) => {
            log::error!("neteq_create: invalid NetEqConfig: {e:?}");
            return std::ptr::null_mut();
        }
    };
    let dec = match OpusDecoder::new(config.sample_rate, config.channels) {
        Ok(d) => Box::new(d),
        Err(e) => {
            log::error!("neteq_create: failed to create OpusDecoder: {e:?}");
            return std::ptr::null_mut();
        }
    };
    let handle = Box::new(NetEqHandle {
        neteq: Mutex::new(neteq),
        sample_rate: config.sample_rate,
        channels: config.channels,
    });
    handle.neteq.lock().unwrap().register_decoder(111, dec);
    let ptr: *mut NetEqHandle = Box::into_raw(handle);
    ptr as *mut c_void
}

#[no_mangle]
pub extern "C" fn neteq_destroy(neteq_ptr: *mut c_void) {
    unsafe {
        drop(Box::from_raw(neteq_ptr as *mut NetEqHandle));
    }
}

// Returns 10ms audio frame of signed float32
// ret_buf must be (sample_rate/100)*channels*sizeof(float) bytes, per the configured NetEqConfigFfi
#[no_mangle]
pub extern "C" fn neteq_get_audio_frame(neteq_ptr: *mut c_void, ret_buf: *mut f32) {
    let handle: &mut NetEqHandle = unsafe { &mut *(neteq_ptr as *mut NetEqHandle) };
    let expected = (handle.sample_rate / 100) * handle.channels as u32;
    // get_audio() appears to handle underflow, whereas neteq_player.rs explicitly
    // handles errors with "fill silence and return"
    let frame = handle.neteq.lock().unwrap().get_audio().expect("get_audio");
    let mut m = frame.samples.len();
    if m as u32 != expected {
        println!("unexpected sample len {}", m);
        m = min(m, expected as usize);
    }
    for i in 0..m {
        unsafe {
            *ret_buf.add(i) = frame.samples[i];
        }
    }
}

// Insert 20ms of RTP Opus audio using the sample_rate/channels configured in neteq_create.
// payload is variable length because this is compressed Opus (not PCM).
#[no_mangle]
pub extern "C" fn neteq_insert_audio_packet(
    neteq_ptr: *mut c_void,
    sequence_number: u16,
    timestamp: u32,
    payload: *mut u8,
    payload_len: u32,
) {
    let ssrc = 12345;
    let hdr = RtpHeader::new(sequence_number, timestamp, ssrc, 111, false);
    let vec: Vec<u8>;
    unsafe {
        let len: usize = payload_len.try_into().unwrap();
        let slice = std::slice::from_raw_parts(payload, len);
        vec = slice.to_vec();
    }
    let handle: &mut NetEqHandle = unsafe { &mut *(neteq_ptr as *mut NetEqHandle) };
    let p = AudioPacket::new(hdr, vec, handle.sample_rate, handle.channels, 20);
    handle
        .neteq
        .lock()
        .unwrap()
        .insert_packet(p)
        .expect("insert_packet");
}
