//! Opt-in commissioning endpoint: control through RPMsg, two independent DDR
//! streams. The producer is deterministic TEST DATA, not a sensor/AI workload.
use super::*;
#[cfg(any(test,feature="amp-observation"))]
use crate::local_queue::Consumer;
#[cfg(any(test,feature="amp-observation"))]
use rp1_abi::observation as obs;

const HELLO: u32 = 0;
const TX_START: u32 = 3;
const QUERY_RX: u32 = 4;
const QUERY_TX: u32 = 5;
const RETIRE: u32 = 6;
const QUERY_CONTROL: u32 = 10;
const REPLY_MAGIC: u32 = u32::from_le_bytes(*b"BKR1");
const COPY_WORDS: usize = 4; // 16 bytes/direction/pass; qualify stack before increasing.

struct Endpoint {
    #[cfg(not(any(test,feature="amp-observation")))]
    pump: Option<Duplex>,
    #[cfg(any(test,feature="amp-observation"))]
    pump: Option<observation::Collector<'static,8>>,
    peer: u32, rx_sum: u32,
    #[cfg(any(test,feature="amp-observation"))]
    retired: bool,
}
impl Endpoint {
    const fn empty() -> Self { Self { pump: None, peer: 0, rx_sum: 0,
        #[cfg(any(test,feature="amp-observation"))] retired: false,
    } }
    fn pump_ref(&self) -> Option<&Duplex> {
        #[cfg(any(test,feature="amp-observation"))]
        {self.pump.as_ref().map(|c|&c.pump)}
        #[cfg(not(any(test,feature="amp-observation")))]
        {self.pump.as_ref()}
    }
    fn pump_mut(&mut self) -> Option<&mut Duplex> {
        #[cfg(any(test,feature="amp-observation"))]
        {self.pump.as_mut().map(|c|&mut c.pump)}
        #[cfg(not(any(test,feature="amp-observation")))]
        {self.pump.as_mut()}
    }
    #[cfg(any(test,feature="amp-observation"))]
    fn collector_ref(&self)->Option<&observation::Collector<'static,8>> {
        self.pump.as_ref().filter(|c|c.attached())
    }
    fn dead(&self) -> bool {
        #[cfg(any(test,feature="amp-observation"))] {self.retired}
        #[cfg(not(any(test,feature="amp-observation")))] {self.pump.as_ref().is_some_and(|p|p.retired)}
    }
    fn retire(&mut self) {
        if let Some(p) = self.pump_mut() { p.retire(); }
        #[cfg(any(test,feature="amp-observation"))]
        {self.retired=true;}
    }
    fn dispatch(&mut self, packet: &[u8], peer: u32, boot: u64, now: u64,
        claim: impl FnOnce() -> Option<ReservedDdrWindow>) -> Result<(), u32> {
        if self.dead() { return Err(6); }
        let c = Command::decode(packet).map_err(|_| 1u32)?;
        if c.generation().boot != boot {return Err(2);}
        if c.apply_time() > now { return Err(4); }
        let mut d = Descriptor::decode(&packet[64..]).map_err(|_| 1u32)?;
        let kind = c.0[12];
        if kind == HELLO {
            if self.pump_ref().is_some() || c.generation().boot != boot || c.sequence() != 1 || d.0 != [0;16] {
                return Err(2);
            }
            let mut gate = CommandGate::new(c.generation()).map_err(|_| 2u32)?;
            gate.accept(c, 64, now).map_err(|e| code(e.into()))?;
            // Claim is invoked only after the entire HELLO has been validated.
            // It hands over a unique, verified fixed window, not an address.
            let window = claim().ok_or(9u32)?;
            let mut pump = Duplex::new(window, c.generation()).map_err(code)?;
            pump.control = gate;
            #[cfg(any(test,feature="amp-observation"))]
            {self.pump=Some(observation::Collector::commissioning(pump));}
            #[cfg(not(any(test,feature="amp-observation")))]
            {self.pump=Some(pump);}
            self.peer = peer; self.rx_sum = 0;
            return Ok(());
        }
        if peer != self.peer { return Err(2); }
        let mut gate = self.pump_ref().ok_or(2u32)?.control;
        gate.accept(c, 64, now).map_err(|e| code(e.into()))?;
        match kind {
            RX_READY => { self.pump_mut().unwrap().accept_rx(d).map_err(code)?; self.rx_sum = 0; }
            TX_FREE => self.pump_mut().unwrap().acknowledge_tx(d).map_err(code)?,
            TX_START => {
                #[cfg(any(test,feature="amp-observation"))]
                if self.collector_ref().is_some() {return Err(1);}
                // Commissioning producer timestamps its own acquisition. Linux
                // cannot assert a fake acquisition time or synchronization fit.
                if d.0[11] | d.0[12] | d.0[13] != 0 || d.0[14] != u32::MAX { return Err(1); }
                d.0[11] = now as u32; d.0[12] = (now >> 32) as u32;
                self.pump_mut().unwrap().begin_tx(d).map_err(code)?;
            }
            QUERY_RX | QUERY_TX | RETIRE => { if d.0 != [0;16] { return Err(1); } }
            #[cfg(any(test,feature="amp-observation"))]
            obs::QUERY => {if d.0 != [0;16] {return Err(1);} if self.collector_ref().is_none() {return Err(9);}}
            _ => return Err(1),
        }
        self.pump_mut().unwrap().control = gate;
        if kind == RETIRE {
            let completion=self.pump_mut().unwrap().quiesce_for_peer();
            self.retire();
            completion.map_err(code)?;
        }
        Ok(())
    }
    /// Rewrite only the received 128-byte payload; OpenAMP retains it when TX
    /// buffers are full. Queue retries never execute the command a second time.
    fn receive(&mut self, packet: &mut [u8], peer: u32, boot: u64, now: u64,
        claim: impl FnOnce() -> Option<ReservedDdrWindow>) -> usize {
        if packet.len() != 128 { return 0; }
        let kind = u32::from_le_bytes(packet[48..52].try_into().unwrap());
        let sequence: [u8;8] = packet[24..32].try_into().unwrap();
        let status = self.dispatch(packet, peer, boot, now, claim).err().unwrap_or(0);
        self.reply(packet, kind, sequence, status, now)
    }
    fn reply(&self, packet: &mut [u8], kind: u32, sequence: [u8;8], status: u32, now: u64) -> usize {
        packet.fill(0);
        let (drops_rx, drops_tx) = self.pump_ref().map_or((0,0), Duplex::drops);
        let mode=0;
        #[cfg(any(test,feature="amp-observation"))]
        let mode=if status==0 && self.collector_ref().is_some() {obs::STREAM_REPLY} else {mode};
        let header = [REPLY_MAGIC,1,status,kind,0,0,drops_rx as u32,(drops_rx>>32) as u32,
            drops_tx as u32,(drops_tx>>32) as u32,self.rx_sum,u32::from(self.dead()),
            now as u32,(now>>32) as u32,COPY_WORDS as u32,mode];
        for (out, value) in packet.chunks_exact_mut(4).zip(header) { out.copy_from_slice(&value.to_le_bytes()); }
        packet[16..24].copy_from_slice(&sequence);
        if let Some(p) = self.pump_ref() {
            let direction = if matches!(kind, TX_FREE | TX_START | QUERY_TX) { &p.tx } else { &p.rx };
            if let Some(mut descriptor) = direction.descriptor {
                descriptor.0[1] = direction.slot.state() as u32;
                for (out, value) in packet[64..].chunks_exact_mut(4).zip(descriptor.0) { out.copy_from_slice(&value.to_le_bytes()); }
            }
        }
        128
    }
    #[cfg(any(test,feature="amp-proc1-probe"))]
    fn receive_control(&mut self, packet:&mut[u8], peer:u32, boot:u64, now:u64,
        control:impl FnOnce(&[u8],u64)->Result<[u32;16],u32>,
        observation_time:impl FnOnce()->Option<u64>)->Option<usize> {
        use crate::local_control::{SET_PARAMETERS, STOP};
        if packet.len()!=128 { return None; }
        let kind=u32::from_le_bytes(packet[48..52].try_into().unwrap());
        if !matches!(kind,SET_PARAMETERS|STOP|QUERY_CONTROL) { return None; }
        let sequence=packet[24..32].try_into().unwrap();
        let mut result=(|| {
            if self.dead() { return Err(6); }
            if peer!=self.peer { return Err(2); }
            let p=self.pump_mut().ok_or(2u32)?;
            let c=Command::decode(packet).map_err(|_|1u32)?;
            if c.generation().boot!=boot { return Err(2); }
            if kind==QUERY_CONTROL && (c.apply_time()>now || packet[64..].iter().any(|b|*b!=0)) {return Err(1);}
            let mut gate=p.control;
            gate.accept(c,64,now).map_err(|e|code(e.into()))?;
            let receipt=control(packet,now)?;
            p.control=gate;
            Ok(receipt)
        })();
        // A pre-copy admission time cannot bound a concurrent C1 receipt.
        let mut reply_time=now;
        if kind==QUERY_CONTROL && result.is_ok() {
            match observation_time() {
                Some(t) if t>=now=>reply_time=t,
                _=>{self.retire();result=Err(8);}
            }
        }
        let status=result.as_ref().err().copied().unwrap_or(0);
        self.reply(packet,kind,sequence,status,reply_time);
        packet[64..].fill(0);
        if let Ok(receipt)=result {
            packet[60..64].copy_from_slice(&(if kind==QUERY_CONTROL {3u32} else {1u32}).to_le_bytes());
            for (out,value) in packet[64..].chunks_exact_mut(4).zip(receipt) {out.copy_from_slice(&value.to_le_bytes());}
        }
        Some(128)
    }
    fn poll(&mut self) -> Result<(), Error> {
        if self.dead() { return Ok(()); }
        #[cfg(any(test,feature="amp-observation"))]
        if let Some(c)=self.pump.as_mut().filter(|c|c.attached()) {
            if c.pump.retired {return Ok(());}
            let rx=(|| {
                if c.pump.rx.slot.state()!=State::Reading {return Ok(());}
                let d=c.pump.rx.descriptor.ok_or(AbiError::Owner)?;
                let n=((d.0[10] as usize-c.pump.rx.cursor)/4).min(COPY_WORDS);
                let mut words=[0;COPY_WORDS];c.read_rx(&mut words[..n])?;
                for word in &words[..n] {self.rx_sum=self.rx_sum.wrapping_add(*word);}
                if c.pump.rx.cursor==d.0[10] as usize {c.release_rx()?;}
                Ok::<(),Error>(())
            })();
            if rx.is_err() {c.retire();} else {let _=c.poll();}
            // A telemetry fault retires only its DDR path; C1 and commands keep running.
            return Ok(());
        }
        #[cfg(any(test,feature="amp-observation"))]
        let Some(p)=self.pump.as_mut().map(|c|&mut c.pump) else {return Ok(());};
        #[cfg(not(any(test,feature="amp-observation")))]
        let Some(p) = &mut self.pump else { return Ok(()); };
        let mut words = [0;COPY_WORDS];
        if p.rx.slot.state() == State::Reading {
            let d = p.rx.descriptor.ok_or(AbiError::Owner)?;
            let n = ((d.0[10] as usize - p.rx.cursor)/4).min(COPY_WORDS);
            p.read_rx(&mut words[..n])?;
            for word in &words[..n] { self.rx_sum = self.rx_sum.wrapping_add(*word); }
            if p.rx.cursor == d.0[10] as usize { p.release_rx()?; }
        }
        if p.tx.slot.state() == State::Writing {
            let d = p.tx.descriptor.ok_or(AbiError::Owner)?;
            let first = p.tx.cursor/4;
            let n = ((d.0[10] as usize - p.tx.cursor)/4).min(COPY_WORDS);
            for (i, word) in words[..n].iter_mut().enumerate() { *word = d.0[6].wrapping_add((first+i) as u32) ^ 0xa55aa55a; }
            p.write_tx(&words[..n])?;
            if p.tx.cursor == d.0[10] as usize { p.publish_tx()?; }
        }
        Ok(())
    }
    #[cfg(any(test,feature="amp-observation"))]
    fn receive_observations(&mut self,packet:&mut[u8],peer:u32,boot:u64,now:u64,
        take:impl FnOnce()->Option<Consumer<'static,obs::Observation,8>>)->Option<usize> {
        if packet.len()!=128 {return None;}
        let kind=u32::from_le_bytes(packet[48..52].try_into().unwrap());
        if kind!=obs::START && kind!=obs::QUERY {return None;}
        let sequence=packet[24..32].try_into().unwrap();
        let status=(|| {
            if kind==obs::QUERY {return self.dispatch(packet,peer,boot,now,||None);}
            if self.dead() {return Err(6);}
            if peer!=self.peer {return Err(2);}
            let command=Command::decode(packet).map_err(|_|1u32)?;
            if command.generation().boot!=boot {return Err(2);}
            if command.apply_time()>now {return Err(4);}
            if packet[64..].iter().any(|b|*b!=0) {return Err(1);}
            let p=self.pump_ref().ok_or(2u32)?;
            let mut gate=p.control;gate.accept(command,64,now).map_err(|e|code(e.into()))?;
            if self.collector_ref().is_some() {return Err(5);}
            if p.retired || p.rx.slot.state()!=State::Free || p.tx.descriptor.is_some() {return Err(5);}
            let consumer=take().ok_or(9u32)?;
            let c=self.pump.as_mut().unwrap();c.pump.control=gate;c.attach(consumer);
            Ok(())
        })().err().unwrap_or(0);
        self.reply(packet,kind,sequence,status,now);
        if status==0 && kind==obs::QUERY {
            let c=self.collector_ref().unwrap();let counters=c.counters();let g=c.pump.generation;
            let data=[obs::COUNTER_MAGIC,rp1_abi::amp::VERSION,g.boot as u32,(g.boot>>32) as u32,g.session as u32,(g.session>>32) as u32,
                counters.published as u32,(counters.published>>32) as u32,counters.faults as u32,(counters.faults>>32) as u32,
                counters.source_drop_newest,obs::BYTES as u32,u32::from(c.pump.retired),0,0,0];
            packet[60..64].copy_from_slice(&obs::COUNTER_REPLY.to_le_bytes());
            for (out,value) in packet[64..].chunks_exact_mut(4).zip(data) {out.copy_from_slice(&value.to_le_bytes());}
        }
        Some(128)
    }
    #[cfg(any(test,feature="amp-proc1-probe"))]
    fn receive_with_local_session(&mut self,packet:&mut[u8],peer:u32,boot:u64,now:u64,
        claim:impl FnOnce()->Option<ReservedDdrWindow>,begin:impl FnOnce(Generation)->u32,
        mut retire:impl FnMut())->usize {
        let was_dead=self.dead();
        let original=Command::decode(packet).ok();
        let length=self.receive(packet,peer,boot,now,claim);
        if length==128 && u32::from_le_bytes(packet[8..12].try_into().unwrap())==0 {
            if let Some(command)=original {
                if command.0[12]==HELLO {
                    let status=begin(command.generation());
                    if status!=0 {self.retire();packet[8..12].copy_from_slice(&status.to_le_bytes());packet[44..48].copy_from_slice(&1u32.to_le_bytes());}
                }
            }
        }
        if !was_dead && self.dead() {retire();}
        length
    }
}

fn code(e: Error) -> u32 {
    match e {
        Error::Abi(AbiError::Full) => 5,
        Error::Abi(AbiError::Generation | AbiError::Owner) => 2,
        Error::Abi(AbiError::Sequence) => 3,
        Error::Abi(AbiError::Expired | AbiError::Time) => 4,
        Error::Abi(AbiError::Bounds | AbiError::Alignment | AbiError::Overflow | AbiError::Width) => 7,
        Error::Abi(_) => 1,
        Error::Retired => 6,
        _ => 8,
    }
}

#[cfg(feature = "amp-bulk")]
mod hardware {
    use super::*;
    use core::ptr;
    #[used]
    #[cfg_attr(not(feature="amp-observation"),unsafe(link_section = ".openamp_state"))]
    #[cfg_attr(feature="amp-observation",unsafe(link_section = ".amp_observation_state"))]
    static mut ENDPOINT: Endpoint = Endpoint::empty();
    unsafe extern "C" { fn rp1_bulk_claim_window() -> bool; }
    unsafe fn endpoint() -> &'static mut Endpoint { unsafe { &mut *ptr::addr_of_mut!(ENDPOINT) } }
    fn now() -> Option<u64> {
        for _ in 0..4 {
            let hi = unsafe { (0x400ac024 as *const u32).read_volatile() };
            let lo = unsafe { (0x400ac028 as *const u32).read_volatile() };
            if hi == unsafe { (0x400ac024 as *const u32).read_volatile() } { return Some((u64::from(hi)<<32)|u64::from(lo)); }
        }
        None
    }
    // All hooks belong to the single OpenAMP worker. No IRQ/task may call them.
    #[unsafe(no_mangle)]
    unsafe extern "C" fn rp1_bulk_cold_reset() { unsafe { ptr::addr_of_mut!(ENDPOINT).write(Endpoint::empty()); } }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn rp1_bulk_quiesce() { unsafe { endpoint().retire(); } #[cfg(feature="amp-proc1-probe")] unsafe { rp1_control_retire(); } }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn rp1_bulk_receive(data: *mut u8, len: usize, peer: u32) -> usize {
        if len != 128 { return 0; }
        let e = unsafe { endpoint() };
        let Some(t) = now() else { e.retire(); #[cfg(feature="amp-proc1-probe")] unsafe { rp1_control_retire(); } return 0; };
        let boot = unsafe { (0x2000da70 as *const u32).read_volatile() as u64 |
            (((0x2000da74 as *const u32).read_volatile() as u64)<<32) };
        let claim=|| {
            if unsafe { rp1_bulk_claim_window() } { Some(unsafe { ReservedDdrWindow::from_verified_region0() }) } else { None }
        };
        #[cfg(feature="amp-proc1-probe")]
        unsafe {
            let packet=core::slice::from_raw_parts_mut(data,len);
            #[cfg(feature="amp-observation")]
            if let Some(length)=e.receive_observations(packet,peer,boot,t,||rp1_control_take_trace()) {return length;}
            if let Some(length)=e.receive_control(packet,peer,boot,t,|packet,now| {
                let mut receipt=[0;16];
                let status=rp1_control_message(packet.as_ptr(),now,receipt.as_mut_ptr());
                if status==0 {Ok(receipt)} else {Err(status)}
            },now) {
                if e.dead() {rp1_control_retire();}
                return length;
            }
            return e.receive_with_local_session(packet,peer,boot,t,claim,
                |g|rp1_control_session(g.boot,g.session),||rp1_control_retire());
        }
        #[cfg(not(feature="amp-proc1-probe"))]
        e.receive(unsafe {core::slice::from_raw_parts_mut(data,len)},peer,boot,t,claim)
    }
    #[unsafe(no_mangle)]
    unsafe extern "C" fn rp1_bulk_poll() {
        let e = unsafe { endpoint() };
        if e.poll().is_err() { e.retire(); #[cfg(feature="amp-proc1-probe")] unsafe { rp1_control_retire(); } }
    }

    #[cfg(feature="amp-proc1-probe")]
    unsafe extern "C" {fn rp1_control_session(boot:u64,session:u64)->u32;fn rp1_control_retire();
        fn rp1_control_message(packet:*const u8,now:u64,receipt:*mut u32)->u32;}
    #[cfg(feature="amp-observation")]
    unsafe extern "Rust" {fn rp1_control_take_trace()->Option<Consumer<'static,obs::Observation,8>>;}
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use core::cell::Cell;
    fn packet(kind: u32, seq: u32, d: Descriptor) -> [u8;128] {
        let h: [u32;16] = [1,64,17,0,29,0,seq,0,0,0,100000,0,kind,64,0,0];
        core::array::from_fn(|i| (if i<64 {h[i/4]} else {d.0[(i-64)/4]}).to_le_bytes()[i%4])
    }
    fn word(packet: &[u8], n: usize) -> u32 { u32::from_le_bytes(packet[n*4..n*4+4].try_into().unwrap()) }
    fn desc(actor: Actor, seq: u32) -> Descriptor {
        Descriptor([1,if actor==Actor::Linux {2} else {1},17,0,29,0,seq,0,actor as u32,
            if actor==Actor::Linux {0} else {32768},32768,0,0,0,u32::MAX,0])
    }
    #[cfg(all(target_os="linux",target_pointer_width="64"))]
    pub(crate) fn exercise_observations() {
        extern crate std;
        use crate::{local_queue::LocalQueue,local_control::{LocalControl,Decision,SampleStamp}};
        use observation::tests::ProtectedTx;
        let g=Generation{boot:17,session:29};
        let claim=||Some(unsafe {ReservedDdrWindow::from_verified_region0()});
        // A started commissioning TX cannot switch its producer or claim a queue.
        {
            let mut e=Endpoint::empty();let mut b=packet(HELLO,1,Descriptor([0;16]));
            e.receive(&mut b,42,17,100,claim);
            let mut b=packet(TX_START,2,desc(Actor::Rp1,1));e.receive(&mut b,42,17,100,claim);
            let mut b=packet(obs::START,3,Descriptor([0;16]));
            e.receive_observations(&mut b,42,17,100,||panic!("busy TX claimed queue"));
            assert_eq!(word(&b,2),5);assert!(e.collector_ref().is_none());
        }
        let queue=std::boxed::Box::leak(std::boxed::Box::new(LocalQueue::<obs::Observation,8>::new()));
        let (mut producer,consumer)=queue.split();let mut consumer=Some(consumer);
        let takes=Cell::new(0);let mut e=Endpoint::empty();
        let mut early=packet(obs::START,1,Descriptor([0;16]));
        e.receive_observations(&mut early,42,17,100,||panic!("pre-HELLO queue claim"));
        assert_eq!(word(&early,2),2);
        let mut hello=packet(HELLO,1,Descriptor([0;16]));e.receive(&mut hello,42,17,100,claim);
        for (index,value) in [(0,2),(1,63),(2,18),(4,30),(6,1),(8,101),(10,100),(13,63),(14,1),(15,1),(16,1)] {
            let mut b=packet(obs::START,2,Descriptor([0;16]));
            b[index*4..index*4+4].copy_from_slice(&u32::to_le_bytes(value));
            e.receive_observations(&mut b,42,17,100,||panic!("malformed queue claim"));
            assert_ne!(word(&b,2),0);assert!(e.collector_ref().is_none());
        }
        let mut b=packet(obs::START,2,Descriptor([0;16]));
        e.receive_observations(&mut b,43,17,100,||panic!("wrong peer claimed queue"));assert_eq!(word(&b,2),2);
        let record=|sequence:u64| {
            let t=100+sequence*200;
            obs::Observation {generation:g,sequence,edge_us:0,irq_us:t,transfer_us:t,calculation_us:t,
                actuator_us:t,sample_timestamp_rp1:t,time_sync_generation:0,time_sync_uncertainty_us:u32::MAX,flags:0}
        };
        for sequence in 1..=8 {producer.push(record(sequence)).unwrap();}
        let mut b=packet(obs::START,2,Descriptor([0;16]));
        let mut wrong_boot=packet(QUERY_TX,2,Descriptor([0;16]));
        {let _guard=ProtectedTx::new();e.receive(&mut wrong_boot,42,18,100,||panic!("stale boot window claim"));}
        assert_eq!(word(&wrong_boot,2),2);assert_eq!(e.pump_ref().unwrap().control.last_sequence(),1);
        e.receive_observations(&mut b,42,17,100,||{takes.set(takes.get()+1);consumer.take()});
        assert_eq!(word(&b,2),0);assert_eq!(word(&b,15),obs::STREAM_REPLY);assert_eq!(takes.get(),1);
        assert!(e.collector_ref().is_some());
        let mut duplicate=packet(obs::START,2,Descriptor([0;16]));
        e.receive_observations(&mut duplicate,42,17,100,||panic!("duplicate queue claim"));assert_eq!(word(&duplicate,2),3);
        let mut repeat=packet(obs::START,3,Descriptor([0;16]));
        e.receive_observations(&mut repeat,42,17,100,||panic!("second queue claim"));assert_eq!(word(&repeat,2),5);
        let mut start=packet(TX_START,3,desc(Actor::Rp1,1));e.receive(&mut start,42,17,100,claim);assert_eq!(word(&start,2),1);
        let mut query=packet(obs::QUERY,3,Descriptor([0;16]));
        e.receive_observations(&mut query,42,17,100,||panic!("query claimed queue"));
        assert_eq!(word(&query,2),0);assert_eq!(word(&query,15),obs::COUNTER_REPLY);
        assert_eq!(word(&query,16),obs::COUNTER_MAGIC);assert_eq!(word(&query,22),0);
        for _ in 0..6 {e.poll().unwrap();}
        let payload=||core::array::from_fn::<_,{obs::BYTES},_>(|i|
            unsafe {((0x81008000usize+i/4*4) as *const u32).read_volatile().to_le_bytes()[i%4]});
        let first=payload();assert_eq!(obs::Observation::decode(&first).unwrap(),record(1));
        let mut wire=std::vec::Vec::from(first);
        let mut ready=packet(QUERY_TX,4,Descriptor([0;16]));e.receive(&mut ready,42,17,100,claim);
        assert_eq!(word(&ready,2),0);assert_eq!(word(&ready,15),obs::STREAM_REPLY);
        let mut free=Descriptor::decode(&ready[64..]).unwrap();assert_eq!(free.0[10],96);free.0[1]=0;
        let mut control=LocalControl::new(g,200).unwrap();
        {
            let _guard=ProtectedTx::new();
            for n in 0..100000u64 {
                e.poll().unwrap();
                assert!(matches!(control.step((n+1)*200,SampleStamp{boot:17,sequence:n+1,acquired_us:(n+1)*200}),Decision::Inactive(_)));
                assert_eq!(producer.push(record(n+9)),if n==0 {Ok(())} else {Err(AbiError::Full)});
            }
            assert_eq!(producer.drops(),99999);
            let mut b=packet(QUERY_CONTROL,5,Descriptor([0;16]));
            e.receive_control(&mut b,42,17,100,|_,_|Ok([0;16]),||Some(112));assert_eq!(word(&b,2),0);assert_eq!(word(&b,15),3);
            let mut stale=free;stale.0[4]=30;
            let mut b=packet(TX_FREE,6,stale);e.receive(&mut b,42,17,100,claim);assert_eq!(word(&b,2),2);
            let mut b=packet(TX_FREE,6,free);e.receive(&mut b,42,17,100,claim);assert_eq!(word(&b,2),0);
        }
        assert_eq!(payload(),first);
        let mut sequence=6;
        for sample in 2..=9 {
            for _ in 0..6 {e.poll().unwrap();}
            let bytes=payload();assert_eq!(obs::Observation::decode(&bytes).unwrap(),record(sample));wire.extend_from_slice(&bytes);
            if sample==2 {
                let mut bad=record(10);bad.generation.session=30;producer.push(bad).unwrap();
            }
            let mut free=e.collector_ref().unwrap().ready().unwrap();free.0[1]=0;
            sequence+=1;let mut b=packet(TX_FREE,sequence,free);e.receive(&mut b,42,17,100,claim);assert_eq!(word(&b,2),0);
        }
        let reply;
        {
            let _guard=ProtectedTx::new();e.poll().unwrap();assert!(!e.dead());
            for _ in 0..20 {e.poll().unwrap();}
            sequence+=1;let mut b=packet(obs::QUERY,sequence,Descriptor([0;16]));
            e.receive_observations(&mut b,42,17,100,||panic!("fault query queue claim"));
            assert_eq!(word(&b,2),0);assert_eq!(word(&b,15),obs::COUNTER_REPLY);
            assert_eq!((word(&b,22),word(&b,24),word(&b,26),word(&b,28)),(9,1,99999,1));reply=b;
            sequence+=1;let mut b=packet(QUERY_CONTROL,sequence,Descriptor([0;16]));
            e.receive_control(&mut b,42,17,100,|_,_|Ok([0;16]),||Some(112));assert_eq!(word(&b,2),0);assert_eq!(word(&b,15),3);
            sequence+=1;let mut b=packet(RETIRE,sequence,Descriptor([0;16]));let stops=Cell::new(0);
            e.receive_with_local_session(&mut b,42,17,100,claim,|_|panic!("RETIRE launched C1"),||stops.set(stops.get()+1));
            assert_eq!(word(&b,2),0);assert!(e.dead());assert_eq!(stops.get(),1);
            sequence+=1;let mut b=packet(obs::START,sequence,Descriptor([0;16]));
            e.receive_observations(&mut b,42,17,100,||panic!("retired queue claim"));assert_eq!(word(&b,2),6);
        }
        if let Some(path)=option_env!("RP1_ENDPOINT_OBSERVATION_FIXTURE") {
            std::fs::write(path,wire).unwrap();std::fs::write(std::format!("{path}.reply"),reply).unwrap();
        }
    }
    // Called within the parent test's exclusive anonymous DDR mapping.
    pub(crate) fn exercise() {
        let mut e=Endpoint::empty(); let calls=Cell::new(0);
        let claim=|| { calls.set(calls.get()+1); Some(unsafe {ReservedDdrWindow::from_verified_region0()}) };
        for (index,value) in [(0,2),(1,63),(4,0),(6,0),(8,101),(10,100),(13,63),(14,1),(15,1),(16,1)] {
            let mut malformed=packet(HELLO,1,Descriptor([0;16]));
            malformed[index*4..index*4+4].copy_from_slice(&u32::to_le_bytes(value));
            assert_eq!(e.receive(&mut malformed,42,17,100,claim),128);
            assert_ne!(word(&malformed,2),0); assert_eq!(calls.get(),0);
        }
        let mut short=[0u8;127];assert_eq!(e.receive(&mut short,42,17,100,claim),0);assert_eq!(calls.get(),0);
        let mut bad=packet(HELLO,1,Descriptor([0;16]));
        assert_eq!(e.receive(&mut bad,42,18,100,claim),128); assert_eq!(word(&bad,2),2); assert_eq!(calls.get(),0);
        let mut hello=packet(HELLO,1,Descriptor([0;16]));
        e.receive(&mut hello,42,17,100,claim); assert_eq!(word(&hello,2),0); assert_eq!(calls.get(),1);
        for i in 0..8192 { unsafe { ((0x81000000usize+i*4) as *mut u32).write_volatile(i as u32); } }
        let mut rx=packet(RX_READY,2,desc(Actor::Linux,1));
        let mut wrong=rx; e.receive(&mut wrong,43,17,100,claim); assert_eq!(word(&wrong,2),2);
        e.receive(&mut rx,42,17,100,claim); assert_eq!(word(&rx,2),0);
        let mut tx=packet(TX_START,3,desc(Actor::Rp1,1));
        e.receive(&mut tx,42,17,123,claim); assert_eq!(word(&tx,2),0);
        for _ in 0..2048 { e.poll().unwrap(); }
        let mut qr=packet(QUERY_RX,4,Descriptor([0;16])); e.receive(&mut qr,42,17,124,claim);
        assert_eq!(word(&qr,2),0); assert_eq!(word(&qr,17),0); assert_eq!(word(&qr,10),(0..8192u32).sum());
        let mut qt=packet(QUERY_TX,5,Descriptor([0;16])); e.receive(&mut qt,42,17,124,claim);
        assert_eq!(word(&qt,2),0); assert_eq!(word(&qt,17),2); assert_eq!(word(&qt,27),123);
        for i in 0..8192 { assert_eq!(unsafe { ((0x81008000usize+i*4) as *const u32).read_volatile() }, (1+i as u32)^0xa55aa55a); }
        for _ in 0..100 {
            let mut full=packet(TX_START,6,desc(Actor::Rp1,2)); e.receive(&mut full,42,17,124,claim);
            assert_eq!(word(&full,2),5); e.poll().unwrap();
        }
        let mut ack=Descriptor::decode(&qt[64..]).unwrap(); ack.0[1]=0;
        let mut release=packet(TX_FREE,6,ack); e.receive(&mut release,42,17,124,claim);
        assert_eq!(word(&release,2),0); assert_eq!(word(&release,8),100);
        let mut stop=packet(RETIRE,7,Descriptor([0;16])); e.receive(&mut stop,42,17,124,claim);
        assert_eq!(word(&stop,2),0); assert_eq!(word(&stop,11),1); e.poll().unwrap();
        let mut restart=packet(HELLO,1,Descriptor([0;16])); e.receive(&mut restart,42,17,124,claim);
        assert_eq!(word(&restart,2),6); assert_eq!(calls.get(),1);
        // The local session hook follows admission and is never a second command executor.
        let starts=Cell::new(0);let stops=Cell::new(0);
        let start=|g|{assert_eq!(g,Generation{boot:17,session:29});starts.set(starts.get()+1);0};
        let stop=||stops.set(stops.get()+1);
        let mut e=Endpoint::empty();let mut wrong=packet(HELLO,1,Descriptor([0;16]));
        e.receive_with_local_session(&mut wrong,42,18,100,claim,start,stop);
        assert_ne!(word(&wrong,2),0);assert_eq!(starts.get(),0);
        let mut short=[0;127];assert_eq!(e.receive_with_local_session(&mut short,42,17,100,claim,start,stop),0);
        let mut hello=packet(HELLO,1,Descriptor([0;16]));e.receive_with_local_session(&mut hello,42,17,100,claim,start,stop);
        assert_eq!(word(&hello,2),0);assert_eq!(starts.get(),1);
        let mut query=packet(QUERY_RX,2,Descriptor([0;16]));e.receive_with_local_session(&mut query,42,17,100,claim,start,stop);
        assert_eq!(word(&query,2),0);assert_eq!(starts.get(),1);
        let mut retire=packet(RETIRE,3,Descriptor([0;16]));e.receive_with_local_session(&mut retire,42,17,100,claim,start,stop);
        assert_eq!(word(&retire,2),0);assert_eq!(stops.get(),1);
        let mut restart=packet(HELLO,1,Descriptor([0;16]));e.receive_with_local_session(&mut restart,42,17,100,claim,start,stop);
        assert_eq!(word(&restart,2),6);assert_eq!(starts.get(),1);
        let mut e=Endpoint::empty();let mut hello=packet(HELLO,1,Descriptor([0;16]));
        e.receive_with_local_session(&mut hello,42,17,100,claim,|_|9,stop);
        assert_eq!(word(&hello,2),9);assert_eq!(word(&hello,11),1);assert_eq!(stops.get(),2);
        let mut retry=packet(HELLO,1,Descriptor([0;16]));e.receive_with_local_session(&mut retry,42,17,100,claim,start,stop);
        assert_eq!(word(&retry,2),6);assert_eq!(starts.get(),1);
        let mut e=Endpoint::empty();let mut hello=packet(HELLO,1,Descriptor([0;16]));
        e.receive_with_local_session(&mut hello,42,17,100,claim,start,stop);
        let hooks=Cell::new(0);
        let hook=|_:&[u8],_:u64| {hooks.set(hooks.get()+1);Ok([123;16])};
        for (index,value,peer,boot) in [(2,18,42,17),(4,30,42,17),(0,2,42,17),
            (13,63,42,17),(14,1,42,17),(10,100,42,17),(6,1,42,17),(2,17,43,17),(2,17,42,18)] {
            let mut b=packet(8,2,Descriptor([0;16]));b[index*4..index*4+4].copy_from_slice(&u32::to_le_bytes(value));
            assert_eq!(e.receive_control(&mut b,peer,boot,100,hook,||panic!("rejected command read clock")),Some(128));
            assert_ne!(word(&b,2),0);assert_eq!(hooks.get(),0);
        }
        let mut b=packet(8,2,Descriptor([0;16]));
        e.receive_control(&mut b,42,17,100,|_,_|Err(5),||panic!("full command read clock"));assert_eq!(word(&b,2),5);
        // Rejected queue admission does not consume sequence; success is QUEUED.
        let mut b=packet(8,2,Descriptor([0;16]));e.receive_control(&mut b,42,17,100,hook,||panic!("SET read observation clock"));
        assert_eq!((word(&b,2),word(&b,15),word(&b,16)),(0,1,123));
        let mut b=packet(8,2,Descriptor([0;16]));e.receive_control(&mut b,42,17,100,hook,||panic!("duplicate read clock"));
        assert_eq!(word(&b,2),3);assert_eq!(hooks.get(),1);
        let mut b=packet(QUERY_CONTROL,3,Descriptor([0;16]));b[64]=1;
        e.receive_control(&mut b,42,17,100,hook,||panic!("malformed query read clock"));assert_ne!(word(&b,2),0);assert_eq!(hooks.get(),1);
        let mut b=packet(QUERY_CONTROL,3,Descriptor([0;16]));e.receive_control(&mut b,42,17,100,hook,||Some(112));
        assert_eq!((word(&b,2),word(&b,15),word(&b,12),word(&b,13)),(0,3,112,0));assert_eq!(hooks.get(),2);
        e.retire();let mut b=packet(8,4,Descriptor([0;16]));e.receive_control(&mut b,42,17,100,hook,||panic!("retired query read clock"));
        assert_eq!(word(&b,2),6);assert_eq!(hooks.get(),2);

        // C1 can expire at111 after C0's admission stamp100, before the copy.
        let mut e=Endpoint::empty();let mut hello=packet(HELLO,1,Descriptor([0;16]));
        e.receive(&mut hello,42,17,100,claim);
        let copied=Cell::new(false);let clock_reads=Cell::new(0);
        let mut b=packet(QUERY_CONTROL,2,Descriptor([0;16]));
        e.receive_control(&mut b,42,17,100,|_,t| {
            assert_eq!(t,100);copied.set(true);
            let mut receipt=[0;16];receipt[9]=1;Ok(receipt)
        },|| {assert!(copied.get());clock_reads.set(clock_reads.get()+1);Some(112)});
        assert_eq!(clock_reads.get(),1);
        assert_eq!((word(&b,2),word(&b,15),word(&b,12),word(&b,25)),(0,3,112,1));
        assert!(word(&b,12)>=110); // Deadline110; admission100 is not reused.

        for observed in [None,Some(99)] {
            let mut e=Endpoint::empty();let mut hello=packet(HELLO,1,Descriptor([0;16]));
            e.receive(&mut hello,42,17,100,claim);
            let mut b=packet(QUERY_CONTROL,2,Descriptor([0;16]));
            e.receive_control(&mut b,42,17,100,hook,||observed);
            assert!(e.dead());assert_eq!((word(&b,2),word(&b,11),word(&b,15)),(8,1,0));
            assert!(b[64..].iter().all(|v|*v==0));
            let mut retry=packet(QUERY_CONTROL,3,Descriptor([0;16]));
            e.receive_control(&mut retry,42,17,100,|_,_|panic!("retired hook"),||panic!("retired clock"));
            assert_eq!(word(&retry,2),6);
        }
    }
}
