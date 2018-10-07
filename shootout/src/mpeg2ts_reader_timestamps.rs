use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::pes;
use mpeg2ts_reader::StreamType;

pub struct Mpeg2tsReader {
    demux: demultiplex::Demultiplex<NullDemuxContext>,
    ctx: NullDemuxContext,
}
impl Mpeg2tsReader {
    pub fn new() -> Mpeg2tsReader {
        let mut ctx = NullDemuxContext::new(NullStreamConstructor);
        let demux = demultiplex::Demultiplex::new(&mut ctx);
        Mpeg2tsReader {
            ctx,
            demux,
        }
    }
    pub fn check_timestamps(&mut self, buf: &[u8]) {
        self.demux.push(&mut self.ctx, &buf[..]);
    }
}

packet_filter_switch!{
        NullFilterSwitch<NullDemuxContext> {
            Pat: demultiplex::PatPacketFilter<NullDemuxContext>,
            Pmt: demultiplex::PmtPacketFilter<NullDemuxContext>,
            Null: demultiplex::NullPacketFilter<NullDemuxContext>,
            NullPes: pes::PesPacketFilter<NullDemuxContext,TimeCheckElementaryStreamConsumer>,
        }
    }
demux_context!(NullDemuxContext, NullStreamConstructor);

pub struct NullStreamConstructor;
impl demultiplex::StreamConstructor for NullStreamConstructor {
    type F = NullFilterSwitch;

    fn construct(&mut self, req: demultiplex::FilterRequest) -> Self::F {
        match req {
            demultiplex::FilterRequest::ByPid(0) => NullFilterSwitch::Pat(demultiplex::PatPacketFilter::default()),
            demultiplex::FilterRequest::ByStream(StreamType::H264, pmt_section, stream_info) => TimeCheckElementaryStreamConsumer::construct(pmt_section, stream_info),
            demultiplex::FilterRequest::Pmt{pid, program_number} => NullFilterSwitch::Pmt(demultiplex::PmtPacketFilter::new(pid, program_number)),
            _ => NullFilterSwitch::Null(demultiplex::NullPacketFilter::default()),
        }
    }
}

pub struct TimeCheckElementaryStreamConsumer {
    last_ts: Option<u64>,
}
impl TimeCheckElementaryStreamConsumer {
    fn construct(_pmt_sect: &demultiplex::PmtSection, _stream_info: &demultiplex::StreamInfo) -> NullFilterSwitch {
        let filter = pes::PesPacketFilter::new(TimeCheckElementaryStreamConsumer { last_ts: None });
        NullFilterSwitch::NullPes(filter)
    }
}
impl pes::ElementaryStreamConsumer for TimeCheckElementaryStreamConsumer {
    fn start_stream(&mut self) {  }
    fn begin_packet(&mut self, header: pes::PesHeader) {
        if let pes::PesContents::Parsed(Some(content)) = header.contents() {
            let this_ts = match content.pts_dts() {
                Ok(pes::PtsDts::PtsOnly(Ok(ts))) => Some(ts.value()),
                Ok(pes::PtsDts::Both{ dts: Ok(ts), .. }) => Some(ts.value()),
                _ => None,
            };
            if let Some(this) = this_ts {
                if let Some(last) = self.last_ts {
                    if this <= last {
                        println!("Non-monotonically increasing timestamp, last={}, this={}", last, this);
                    }
                }
                self.last_ts = Some(this);
            }
        }
    }
    fn continue_packet(&mut self, _data: &[u8]) { }
    fn end_packet(&mut self) { }
    fn continuity_error(&mut self) { }
}