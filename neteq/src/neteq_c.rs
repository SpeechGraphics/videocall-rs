use std::cmp::min;
use std::ffi::c_void;
use std::sync::Mutex;

use crate::codec::OpusDecoder;
use crate::neteq::{NetEq, NetEqConfig};
use crate::packet::{AudioPacket, RtpHeader};

#[no_mangle]
pub extern "C" fn neteq_create() -> *mut c_void {
    let config = NetEqConfig {
        sample_rate: 48000,
        channels: 1,
        max_packets_in_buffer: 50,
        max_delay_ms: 500,
        min_delay_ms: 20,
        for_test_no_time_stretching: false,
        ..Default::default()
    };
    let neteq = Box::new(Mutex::new(NetEq::new(config).expect("new")));
    let dec = Box::new(OpusDecoder::new(48000, 1).expect("reg"));
    neteq.lock().unwrap().register_decoder(111, dec);
    let ptr: *mut Mutex<NetEq> = Box::into_raw(neteq);
    ptr as *mut c_void
}

#[no_mangle]
pub extern "C" fn neteq_destroy(neteq_ptr: *mut c_void) {
    unsafe {
        drop(Box::from_raw(neteq_ptr as *mut Mutex<NetEq>));
    }
}

// Returns 10ms audio frame of signed float32
// ret_buf must be 480*sizeof(float) bytes
#[no_mangle]
pub extern "C" fn neteq_get_audio_frame(neteq_ptr: *mut c_void, ret_buf: *mut f32) {
    let neteq: &mut Mutex<NetEq> = unsafe { &mut *(neteq_ptr as *mut Mutex<NetEq>) };
    // get_audio() appears to handle underflow, whereas neteq_player.rs explicitly
    // handles errors with "fill silence and return"
    let frame = neteq.lock().unwrap().get_audio().expect("get_audio");
    let mut m = frame.samples.len();
    if m != 480 {
        println!("unexpected sample len {}", m);
        m = min(m, 480);
    }
    for i in 0..m {
        unsafe {
            *ret_buf.add(i) = frame.samples[i];
        }
    }
}

// Insert 20ms of 1-channel 48kHz RTP Opus audio.
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
    let p = AudioPacket::new(hdr, vec, 48000, 1, 20);
    let neteq: &mut Mutex<NetEq> = unsafe { &mut *(neteq_ptr as *mut Mutex<NetEq>) };
    neteq
        .lock()
        .unwrap()
        .insert_packet(p)
        .expect("insert_packet");
}
