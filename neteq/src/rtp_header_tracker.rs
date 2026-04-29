use std::collections::VecDeque;

use crate::RtpHeader;

pub struct PacketPosition {
    position: usize,
    header: RtpHeader,
}

pub struct RtpHeaderTracker {
    input_position: usize,
    output_position: usize,
    packet_positions: VecDeque<PacketPosition>,
}

impl RtpHeaderTracker {
    /// Create a new NetEQ instance
    pub fn new() -> Self {
        Self {
            input_position: 0,
            output_position: 0,
            packet_positions: VecDeque::new(),
        }
    }

    pub fn record(&mut self, header: RtpHeader, sample_count: usize) {
        self.packet_positions.push_back(PacketPosition {
            position: self.input_position,
            header,
        });
        self.input_position += sample_count;
    }

    pub fn current_header(&self) -> Option<RtpHeader> {
        self.packet_positions
            .front()
            .map(|packet| packet.header.clone())
    }

    pub fn consume(&mut self, sample_count: usize) {
        self.output_position += sample_count;
        self.truncate_packet_positions();
    }

    fn truncate_packet_positions(&mut self) {
        while self.packet_positions.len() > 1 {
            if let Some(second) = self.packet_positions.get(1) {
                if second.position <= self.output_position {
                    self.packet_positions.pop_front();
                    continue;
                }
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_rtp_header(sequence_number: u16) -> RtpHeader {
        RtpHeader {
            sequence_number,
            timestamp: sequence_number as u32 * 160,
            ssrc: 0,
            payload_type: 0,
            marker: false,
        }
    }

    #[test]
    fn starts_empty() {
        let tracker = RtpHeaderTracker::new();
        assert_eq!(tracker.current_header(), None);
    }

    #[test]
    fn records_single_packet() {
        let mut tracker = RtpHeaderTracker::new();
        let h = new_rtp_header(10);

        tracker.record(h.clone(), 160);

        assert_eq!(tracker.current_header(), Some(h));
    }

    #[test]
    fn preserves_order_of_recorded_headers() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(1), 160);
        tracker.record(new_rtp_header(2), 160);
        tracker.record(new_rtp_header(3), 160);

        assert_eq!(tracker.current_header(), Some(new_rtp_header(1)));
    }

    #[test]
    fn truncate_when_output_passes_first_packet() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(1), 160); // position = 0
        tracker.record(new_rtp_header(2), 160); // position = 160
        tracker.record(new_rtp_header(3), 160); // position = 320

        // Consume enough samples to pass the first packet (>=160)
        tracker.consume(160);

        // First packet should be removed
        assert_eq!(tracker.current_header(), Some(new_rtp_header(2)));
    }

    #[test]
    fn truncate_multiple_packets() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(1), 160); // pos = 0
        tracker.record(new_rtp_header(2), 160); // pos = 160
        tracker.record(new_rtp_header(3), 160); // pos = 320
        tracker.record(new_rtp_header(4), 160); // pos = 480

        // Move output forward past packets 1 and 2
        tracker.consume(320);

        assert_eq!(tracker.current_header(), Some(new_rtp_header(3)));
    }

    #[test]
    fn consuming_less_than_first_packet_keeps_header() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(10), 160);
        tracker.consume(80); // still inside packet 1

        assert_eq!(tracker.current_header(), Some(new_rtp_header(10)));
    }

    #[test]
    fn consuming_exactly_at_boundary_removes_first_packet() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(10), 160);
        tracker.record(new_rtp_header(11), 160);

        tracker.consume(160); // output == next packet position

        assert_eq!(tracker.current_header(), Some(new_rtp_header(11)));
    }

    #[test]
    fn handles_consuming_more_than_all_packets() {
        let mut tracker = RtpHeaderTracker::new();

        tracker.record(new_rtp_header(1), 100);
        tracker.record(new_rtp_header(2), 100);

        tracker.consume(1000); // way past everything

        // truncate only removes packets until only one remains
        assert_eq!(tracker.current_header(), Some(new_rtp_header(2)));
    }
}
