//! Bounded, restart-safe scheduling state for finalized-RPC reconciliation.
//! Arrival is never a release barrier: only an exact finalized reconciliation
//! result can move a slot to MATCHED.

use std::collections::BTreeMap;
use anyhow::{bail, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlotState { Collecting, ReadyToReconcile, RetryPending, Matched, Failed }

#[derive(Clone, Debug)]
pub struct SlotWork { pub state: SlotState, pub first_seen_ms: u64, pub next_attempt_ms: u64, pub attempts: u8, pub identities: usize, pub failure: Option<String> }

pub struct Dispatcher { pub slots: BTreeMap<u64, SlotWork>, pub max_active: usize, pub max_attempts: u8, pub base_backoff_ms: u64, pub peak_active: usize }

impl Dispatcher {
    pub fn new(max_active: usize, max_attempts: u8, base_backoff_ms: u64) -> Result<Self> {
        if max_active == 0 || max_attempts == 0 || base_backoff_ms == 0 { bail!("DISPATCHER_LIMIT_INVALID") }
        Ok(Self { slots: BTreeMap::new(), max_active, max_attempts, base_backoff_ms, peak_active: 0 })
    }
    pub fn observe(&mut self, slot: u64, now_ms: u64) -> Result<()> {
        if !self.slots.contains_key(&slot) && self.active() >= self.max_active { bail!("RECONCILIATION_BACKPRESSURE_CAP") }
        self.slots.entry(slot).and_modify(|work| work.identities += 1).or_insert(SlotWork { state: SlotState::Collecting, first_seen_ms: now_ms, next_attempt_ms: now_ms, attempts: 0, identities: 1, failure: None });
        self.peak_active = self.peak_active.max(self.active()); Ok(())
    }
    pub fn finalized_progress(&mut self, slot: u64, now_ms: u64) -> Result<()> { let work=self.slots.get_mut(&slot).ok_or_else(||anyhow::anyhow!("UNKNOWN_SLOT"))?; if work.state==SlotState::Collecting { work.state=SlotState::ReadyToReconcile; work.next_attempt_ms=now_ms; } Ok(()) }
    pub fn due(&self, now_ms: u64) -> Option<u64> { self.slots.iter().find(|(_,w)| matches!(w.state,SlotState::ReadyToReconcile|SlotState::RetryPending)&&w.next_attempt_ms<=now_ms).map(|(s,_)|*s) }
    pub fn result(&mut self, slot:u64, now_ms:u64, result:&str)->Result<()> { let w=self.slots.get_mut(&slot).ok_or_else(||anyhow::anyhow!("UNKNOWN_SLOT"))?; match result { "MATCHED"=>w.state=SlotState::Matched, "MISSING_FILTERED_IDENTITY"|"RPC_NOT_FINALIZED"=>{w.attempts+=1;if w.attempts>=self.max_attempts{w.state=SlotState::Failed;w.failure=Some(result.into())}else{w.state=SlotState::RetryPending;w.next_attempt_ms=now_ms+self.base_backoff_ms*(1u64<<(w.attempts-1));}}, _=>{w.state=SlotState::Failed;w.failure=Some(result.into())} }; Ok(()) }
    pub fn active(&self)->usize { self.slots.values().filter(|w|matches!(w.state,SlotState::Collecting|SlotState::ReadyToReconcile|SlotState::RetryPending)).count() }
    pub fn drain_completed(&mut self)->Vec<(u64,SlotState)> { let done=self.slots.iter().filter(|(_,w)|matches!(w.state,SlotState::Matched|SlotState::Failed)).map(|(s,w)|(*s,w.state.clone())).collect::<Vec<_>>();for(s,_)in&done{self.slots.remove(s);}done }
}

#[cfg(test)] mod tests { use super::*;
 #[test] fn delayed_identities_retry_without_release(){let mut d=Dispatcher::new(2,3,10).unwrap();d.observe(1,0).unwrap();d.finalized_progress(1,1).unwrap();assert_eq!(d.due(1),Some(1));d.result(1,1,"MISSING_FILTERED_IDENTITY").unwrap();assert_eq!(d.due(10),None);assert_eq!(d.due(11),Some(1));d.result(1,11,"MATCHED").unwrap();assert_eq!(d.drain_completed(),vec![(1,SlotState::Matched)]);}
 #[test] fn saturation_and_hard_errors_fail_closed(){let mut d=Dispatcher::new(1,2,1).unwrap();d.observe(1,0).unwrap();assert!(d.observe(2,0).is_err());d.finalized_progress(1,0).unwrap();d.result(1,0,"EXTRA_FILTERED_IDENTITY").unwrap();assert_eq!(d.drain_completed(),vec![(1,SlotState::Failed)]);}
}
