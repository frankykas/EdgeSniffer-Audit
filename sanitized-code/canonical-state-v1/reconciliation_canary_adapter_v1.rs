//! Offline integration adapter. The live binary calls these transitions around
//! its Yellowstone receiver and finalized RPC worker; no transition releases
//! scientific state, only a durable MATCHED receipt can do that.
use anyhow::{bail, Result};
use crate::finalized_rpc_dispatcher_v1::{Dispatcher, SlotState};

pub struct Adapter { pub dispatcher: Dispatcher, pub source_epoch: String, pub gap: bool, pub receipts: Vec<(u64,String)> }
impl Adapter {
 pub fn new(epoch:&str,max_active:usize)->Result<Self>{if epoch.is_empty(){bail!("SOURCE_EPOCH_MISSING")}Ok(Self{dispatcher:Dispatcher::new(max_active,3,1000)?,source_epoch:epoch.into(),gap:false,receipts:vec![]})}
 pub fn ingest(&mut self,epoch:&str,slot:u64,now:u64)->Result<()> {if self.gap||epoch!=self.source_epoch{bail!("SOURCE_EPOCH_OR_CONTINUITY_BREAK")}self.dispatcher.observe(slot,now)}
 pub fn finalized_progress(&mut self,slot:u64,now:u64)->Result<()> { self.dispatcher.finalized_progress(slot,now) }
 pub fn complete_attempt(&mut self,slot:u64,now:u64,disposition:&str)->Result<()> {self.dispatcher.result(slot,now,disposition)?; if !matches!(disposition,"MISSING_FILTERED_IDENTITY"|"RPC_NOT_FINALIZED"){self.receipts.push((slot,disposition.into()));} Ok(())}
 pub fn mark_gap(&mut self){self.gap=true;}
 pub fn restart(&self,epoch:&str)->Result<Self>{if self.gap||epoch!=self.source_epoch{bail!("RESTART_REFUSES_EPOCH_BRIDGE")}let mut next=Self::new(epoch,self.dispatcher.max_active)?;next.dispatcher=self.dispatcher.clone_for_restart()?;next.receipts=self.receipts.clone();Ok(next)}
}
trait RestartClone {fn clone_for_restart(&self)->Result<Dispatcher>;}
impl RestartClone for Dispatcher {fn clone_for_restart(&self)->Result<Dispatcher>{let mut d=Dispatcher::new(self.max_active,self.max_attempts,self.base_backoff_ms)?;d.slots=self.slots.clone();d.peak_active=self.peak_active;Ok(d)}}
#[cfg(test)] mod tests {use super::*;
 #[test] fn retries_without_arrivals_and_restart_keep_pending(){let mut a=Adapter::new("e",2).unwrap();a.ingest("e",9,0).unwrap();a.finalized_progress(9,1).unwrap();a.complete_attempt(9,1,"MISSING_FILTERED_IDENTITY").unwrap();let mut a=a.restart("e").unwrap();assert_eq!(a.dispatcher.due(1001),Some(9));a.complete_attempt(9,1001,"MATCHED").unwrap();assert_eq!(a.dispatcher.drain_completed(),vec![(9,SlotState::Matched)]);}
 #[test] fn gap_or_epoch_never_completes(){let mut a=Adapter::new("e",1).unwrap();a.mark_gap();assert!(a.ingest("e",1,0).is_err());assert!(Adapter::new("e",1).unwrap().ingest("other",1,0).is_err());}
}
