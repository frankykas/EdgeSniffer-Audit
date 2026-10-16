//! Finalized Pump migration -> PumpSwap creation witness.
//!
//! Port of the strict semantics in scripts/pumpSwapInitDecoderV1.py.  This
//! decoder never treats a first swap or current account state as pool creation.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PUMP: &str="6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMPSWAP: &str="pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
pub const CREATE_POOL:[u8;8]=[233,146,209,142,207,104,64,188];
const CREATE_POOL_EVENT:[u8;8]=[177,49,12,210,160,118,167,116];

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub struct CreationWitness { pub pool:String,pub mint:String,pub quote_mint:String,pub creator:String,pub slot:String,pub signature:String,pub transaction_index:String,pub instruction_path:String,pub event_id:String,pub create_event_sha256:String,pub origin:String }
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CreationQuarantine { pub signature:String,pub reason_code:String }

/// Classifies only transactions for which creation-witness validation is
/// meaningful.  This prevents unrelated or ordinary Pump/PumpSwap activity
/// from being represented as a failed pool creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalizedCreationCandidate {
    NotCandidate,
    StandalonePumpSwapExcluded,
    PumpMigrationCandidate,
}

fn key(value:&Value)->Result<String>{value.as_str().or_else(||value.get("pubkey").and_then(Value::as_str)).map(str::to_owned).context("CREATION_ACCOUNT_KEY_LAYOUT")}
fn keys(tx:&Value)->Result<Vec<String>> { let mut out=tx.pointer("/transaction/message/accountKeys").and_then(Value::as_array).context("CREATION_STATIC_KEYS")?.iter().map(key).collect::<Result<Vec<_>>>()?; for path in ["/meta/loadedAddresses/writable","/meta/loadedAddresses/readonly"] { if let Some(values)=tx.pointer(path).and_then(Value::as_array){out.extend(values.iter().map(key).collect::<Result<Vec<_>>>()?)} }; Ok(out) }
fn bytes(value:&Value)->Result<Vec<u8>>{bs58::decode(value.get("data").and_then(Value::as_str).context("CREATION_INSTRUCTION_DATA")?).into_vec().context("CREATION_DATA_BASE58")}
fn b58(slice:&[u8])->String{bs58::encode(slice).into_string()}
fn u16le(slice:&[u8])->u16{u16::from_le_bytes(slice.try_into().expect("length"))}
fn u64le(slice:&[u8])->u64{u64::from_le_bytes(slice.try_into().expect("length"))}

pub fn classify_finalized_pump_migration_candidate(tx:&Value)->Result<FinalizedCreationCandidate> {
    if !tx.pointer("/meta/err").map(Value::is_null).unwrap_or(false) {
        return Ok(FinalizedCreationCandidate::NotCandidate)
    }
    let keys=keys(tx)?;
    let mut has_pump=false;
    let mut has_create_instruction=false;
    for path in ["/transaction/message/instructions", "/meta/innerInstructions"] {
        if path=="/transaction/message/instructions" {
            for value in tx.pointer(path).and_then(Value::as_array).context("CREATION_OUTER")? {
                let program=value.get("programIdIndex").and_then(Value::as_u64).context("CREATION_OUTER_PROGRAM")? as usize;
                has_pump|=keys.get(program).map(String::as_str)==Some(PUMP);
                if keys.get(program).map(String::as_str)==Some(PUMPSWAP) {
                    let raw=bytes(value)?;
                    has_create_instruction|=raw.starts_with(&CREATE_POOL);
                }
            }
        } else if let Some(groups)=tx.pointer(path).and_then(Value::as_array) {
            for group in groups {
                for value in group.get("instructions").and_then(Value::as_array).context("CREATION_INNER")? {
                    let program=value.get("programIdIndex").and_then(Value::as_u64).context("CREATION_INNER_PROGRAM")? as usize;
                    has_pump|=keys.get(program).map(String::as_str)==Some(PUMP);
                    if keys.get(program).map(String::as_str)==Some(PUMPSWAP) {
                        let raw=bytes(value)?;
                        has_create_instruction|=raw.starts_with(&CREATE_POOL);
                    }
                }
            }
        }
    }
    let mut has_create_event=false;
    let logs=tx.pointer("/meta/logMessages").and_then(Value::as_array).context("CREATION_LOGS")?;
    for log in logs {
        let Some(encoded)=log.as_str().and_then(|value|value.strip_prefix("Program data: ")) else { continue };
        if let Ok(raw)=STANDARD.decode(encoded.trim()) { has_create_event|=raw.starts_with(&CREATE_POOL_EVENT); }
    }
    if !has_create_instruction && !has_create_event { return Ok(FinalizedCreationCandidate::NotCandidate) }
    let migration=has_pump&&logs.iter().filter_map(Value::as_str).any(|value|value.contains("Instruction: Migrate"));
    Ok(if migration { FinalizedCreationCandidate::PumpMigrationCandidate } else { FinalizedCreationCandidate::StandalonePumpSwapExcluded })
}

#[derive(Clone)] struct Instruction { path:String, accounts:Vec<String>, index:u16, base:u64, quote:u64, pool:String,creator:String,mint:String,quote_mint:String,coin_creator:String,mayhem:bool,inner:bool }
#[derive(Clone)] struct Event { log_index:usize,index:u16,creator:String,mint:String,quote_mint:String,base:u64,quote:u64,pool:String,coin_creator:String,mayhem:bool,hash:String }

fn instruction(value:&Value, keys:&[String], path:String, inner:bool)->Result<Option<Instruction>> { let program=value.get("programIdIndex").and_then(Value::as_u64).context("CREATION_PROGRAM_INDEX")? as usize; if keys.get(program).map(String::as_str)!=Some(PUMPSWAP){return Ok(None)} let raw=bytes(value)?; if raw.len()<59||raw[..8]!=CREATE_POOL{return Ok(None)} let accounts=value.get("accounts").and_then(Value::as_array).context("CREATION_ACCOUNTS")?.iter().map(|x|x.as_u64().and_then(|i|keys.get(i as usize)).cloned().context("CREATION_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?; if accounts.len()!=18{bail!("CREATE_POOL_ACCOUNT_LAYOUT")}; if raw[58]>1{bail!("CREATE_POOL_MAYHEM_BOOL")}; Ok(Some(Instruction{path, pool:accounts[0].clone(),creator:accounts[2].clone(),mint:accounts[3].clone(),quote_mint:accounts[4].clone(),accounts,index:u16le(&raw[8..10]),base:u64le(&raw[10..18]),quote:u64le(&raw[18..26]),coin_creator:b58(&raw[26..58]),mayhem:raw[58]!=0,inner})) }
fn event(log:&str,index:usize)->Result<Option<Event>> { let Some(encoded)=log.strip_prefix("Program data: ") else{return Ok(None)}; let raw=match STANDARD.decode(encoded.trim()){Ok(raw)=>raw,Err(_)=>return Ok(None)}; if !raw.starts_with(&CREATE_POOL_EVENT){return Ok(None)}; if raw.len()!=344{bail!("CREATE_POOL_EVENT_LAYOUT")}; Ok(Some(Event{log_index:index,index:u16le(&raw[16..18]),creator:b58(&raw[18..50]),mint:b58(&raw[50..82]),quote_mint:b58(&raw[82..114]),base:u64le(&raw[116..124]),quote:u64le(&raw[124..132]),pool:b58(&raw[173..205]),coin_creator:b58(&raw[301..333]),mayhem:match raw[333]{0=>false,1=>true,_=>bail!("CREATE_POOL_EVENT_MAYHEM_BOOL")},hash:pump_exact_evidence_ledger::sha256_hex(&raw)})) }

pub fn validate_finalized_pump_migration(slot:u64,index:u64,tx:&Value)->Result<Vec<CreationWitness>> { if !tx.pointer("/meta/err").map(Value::is_null).unwrap_or(false){return Ok(vec![])} let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("CREATION_SIGNATURE")?.to_owned(); let keys=keys(tx)?; let mut has_pump=false; let mut instructions=Vec::new(); for (outer,value) in tx.pointer("/transaction/message/instructions").and_then(Value::as_array).context("CREATION_OUTER")?.iter().enumerate(){let program=value.get("programIdIndex").and_then(Value::as_u64).context("CREATION_OUTER_PROGRAM")? as usize;has_pump|=keys.get(program).map(String::as_str)==Some(PUMP);if let Some(i)=instruction(value,&keys,format!("outer:{outer:05}"),false)?{instructions.push(i)}} if let Some(groups)=tx.pointer("/meta/innerInstructions").and_then(Value::as_array){for group in groups {let outer=group.get("index").and_then(Value::as_u64).context("CREATION_INNER_PARENT")?;for (inner,value) in group.get("instructions").and_then(Value::as_array).context("CREATION_INNER")?.iter().enumerate(){let program=value.get("programIdIndex").and_then(Value::as_u64).context("CREATION_INNER_PROGRAM")? as usize;has_pump|=keys.get(program).map(String::as_str)==Some(PUMP);let stack=value.get("stackHeight").and_then(Value::as_u64).context("CREATION_STACK_HEIGHT")?;if let Some(mut i)=instruction(value,&keys,format!("outer:{outer:05}/inner:{inner:05}/stack:{stack:03}"),true)?{i.path=format!("outer:{outer:05}/inner:{inner:05}/stack:{stack:03}");instructions.push(i)}}}}
 let logs=tx.pointer("/meta/logMessages").and_then(Value::as_array).context("CREATION_LOGS")?; let events=logs.iter().enumerate().map(|(i,l)|event(l.as_str().context("CREATION_LOG")?,i)).collect::<Result<Vec<_>>>()?.into_iter().flatten().collect::<Vec<_>>(); if instructions.is_empty()&&events.is_empty(){return Ok(vec![])} if instructions.is_empty() || events.is_empty() { bail!("CREATE_POOL_EVENT_CARDINALITY") } let matches=instructions.iter().flat_map(|i|events.iter().filter(move|e|e.pool==i.pool&&e.creator==i.creator&&e.mint==i.mint&&e.quote_mint==i.quote_mint&&e.index==i.index&&e.base==i.base&&e.quote==i.quote&&e.coin_creator==i.coin_creator&&e.mayhem==i.mayhem).map(move|e|(i,e))).collect::<Vec<_>>(); if matches.len()!=1{bail!("CREATE_POOL_EVENT_CARDINALITY")}; let (i,e)=matches[0]; let migrate=has_pump&&logs.iter().filter_map(Value::as_str).any(|x|x.contains("Instruction: Migrate")); if !(i.inner&&i.index==0&&migrate){return Ok(vec![])} Ok(vec![CreationWitness{pool:i.pool.clone(),mint:i.mint.clone(),quote_mint:i.quote_mint.clone(),creator:i.creator.clone(),slot:slot.to_string(),signature,transaction_index:index.to_string(),instruction_path:i.path.clone(),event_id:format!("{}:{}",tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("CREATION_SIGNATURE")?,e.log_index),create_event_sha256:e.hash.clone(),origin:"PUMP_MIGRATE_CREATE_POOL".into()}]) }

#[cfg(test)]
mod tests {
    use super::*;

    fn b(n: u8) -> String { bs58::encode([n; 32]).into_string() }
    fn create_data() -> String {
        let mut raw = vec![0_u8; 59];
        raw[..8].copy_from_slice(&CREATE_POOL);
        raw[10..18].copy_from_slice(&1000_u64.to_le_bytes());
        raw[18..26].copy_from_slice(&2000_u64.to_le_bytes());
        raw[26..58].copy_from_slice(&[9; 32]);
        bs58::encode(raw).into_string()
    }
    fn event_data(mint: u8) -> String {
        let mut raw = vec![0_u8; 344];
        raw[..8].copy_from_slice(&CREATE_POOL_EVENT);
        raw[18..50].copy_from_slice(&[4; 32]);
        raw[50..82].copy_from_slice(&[mint; 32]);
        raw[82..114].copy_from_slice(&[6; 32]);
        raw[116..124].copy_from_slice(&1000_u64.to_le_bytes());
        raw[124..132].copy_from_slice(&2000_u64.to_le_bytes());
        raw[173..205].copy_from_slice(&[2; 32]);
        raw[301..333].copy_from_slice(&[9; 32]);
        STANDARD.encode(raw)
    }
    fn migration(loaded: bool) -> Value {
        let mut static_keys = vec![PUMPSWAP.to_owned(), PUMP.to_owned()];
        static_keys.extend((2..=6).map(b));
        let loaded_keys = if loaded { (7..=19).map(b).collect::<Vec<_>>() } else { static_keys.extend((7..=19).map(b)); Vec::new() };
        let accounts = (2..20).collect::<Vec<_>>();
        let create = serde_json::json!({"programIdIndex":0,"accounts":accounts,"data":create_data(),"stackHeight":2});
        let mut meta = serde_json::json!({"err":null,"logMessages":[format!("Program {PUMP} invoke [1]"),"Program log: Instruction: Migrate",format!("Program {PUMPSWAP} invoke [2]"),format!("Program data: {}",event_data(5)),format!("Program {PUMPSWAP} success"),format!("Program {PUMP} success")],"innerInstructions":[{"index":0,"instructions":[{"programIdIndex":3,"accounts":[],"data":"","stackHeight":2},create]}]});
        if loaded { meta["loadedAddresses"] = serde_json::json!({"writable":loaded_keys,"readonly":[]}); }
        serde_json::json!({"transaction":{"signatures":["sig"],"message":{"accountKeys":static_keys,"instructions":[{"programIdIndex":1,"accounts":[],"data":""}]}},"meta":meta})
    }
    #[test]
    fn genuine_finalized_migration_admits() {
        let result = validate_finalized_pump_migration(42, 7, &migration(false)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].origin, "PUMP_MIGRATE_CREATE_POOL");
        assert_eq!(result[0].pool, b(2));
        assert_eq!(result[0].mint, b(5));
        assert_eq!(result[0].instruction_path, "outer:00000/inner:00001/stack:002");
    }
    #[test]
    fn loaded_addresses_and_multiple_inner_instructions_admit() {
        assert_eq!(validate_finalized_pump_migration(42, 7, &migration(true)).unwrap().len(), 1);
    }
    #[test]
    fn mismatched_event_quarantines() {
        let mut tx = migration(false);
        tx["meta"]["logMessages"][3] = Value::String(format!("Program data: {}", event_data(8)));
        assert!(validate_finalized_pump_migration(42, 7, &tx).unwrap_err().to_string().contains("CARDINALITY"));
    }
    #[test]
    fn event_without_instruction_is_quarantined_not_silently_lost() {
        let tx=serde_json::json!({"transaction":{"signatures":["s"],"message":{"accountKeys":[],"instructions":[]}},"meta":{"err":null,"logMessages":[format!("Program data: {}",event_data(5)) ]}});
        assert!(validate_finalized_pump_migration(1,1,&tx).unwrap_err().to_string().contains("CARDINALITY"));
    }
    #[test]
    fn instruction_without_event_quarantines() {
        let mut tx = migration(false);
        tx["meta"]["logMessages"] = serde_json::json!(["Program log: Instruction: Migrate"]);
        assert!(validate_finalized_pump_migration(42, 7, &tx).unwrap_err().to_string().contains("CARDINALITY"));
    }
    #[test]
    fn duplicate_witness_evidence_quarantines() {
        let mut tx = migration(false);
        let duplicate = tx["meta"]["logMessages"][3].clone();
        tx["meta"]["logMessages"].as_array_mut().unwrap().push(duplicate);
        assert!(validate_finalized_pump_migration(42, 7, &tx).unwrap_err().to_string().contains("CARDINALITY"));
    }
    #[test]
    fn malformed_metadata_quarantines() {
        let mut tx = migration(false);
        tx["meta"]["innerInstructions"][0]["instructions"][1].as_object_mut().unwrap().remove("stackHeight");
        assert!(validate_finalized_pump_migration(42, 7, &tx).unwrap_err().to_string().contains("STACK_HEIGHT"));
    }
    #[test]
    fn failed_and_standalone_are_excluded() {
        let mut failed=migration(false); failed["meta"]["err"]=serde_json::json!({"InstructionError":[0,"Custom"]});
        assert!(validate_finalized_pump_migration(42,7,&failed).unwrap().is_empty());
        let mut standalone=migration(false); standalone["transaction"]["message"]["instructions"]=serde_json::json!([]); standalone["meta"]["logMessages"]=serde_json::json!([format!("Program data: {}",event_data(5))]);
        // A create/event pair without the Pump migration proof is intentionally excluded.
        assert!(validate_finalized_pump_migration(42,7,&standalone).unwrap().is_empty());
    }
    #[test]
    fn candidate_classifier_never_quarantines_unrelated_or_ordinary_pump_activity() {
        let unrelated=serde_json::json!({"transaction":{"signatures":["u"],"message":{"accountKeys":["other"],"instructions":[]}},"meta":{"err":null,"logMessages":[]}});
        assert_eq!(classify_finalized_pump_migration_candidate(&unrelated).unwrap(),FinalizedCreationCandidate::NotCandidate);
        let ordinary=serde_json::json!({"transaction":{"signatures":["p"],"message":{"accountKeys":[PUMP],"instructions":[]}},"meta":{"err":null,"logMessages":[]}});
        assert_eq!(classify_finalized_pump_migration_candidate(&ordinary).unwrap(),FinalizedCreationCandidate::NotCandidate);
        let mut standalone=migration(false); standalone["transaction"]["message"]["instructions"]=serde_json::json!([]); standalone["meta"]["logMessages"]=serde_json::json!([format!("Program data: {}",event_data(5))]);
        assert_eq!(classify_finalized_pump_migration_candidate(&standalone).unwrap(),FinalizedCreationCandidate::StandalonePumpSwapExcluded);
        let mut invalid=migration(false);invalid["meta"]["logMessages"][3]=Value::String("Program data: not-a-create-event".into());
        assert_eq!(classify_finalized_pump_migration_candidate(&invalid).unwrap(),FinalizedCreationCandidate::PumpMigrationCandidate);
    }
}
