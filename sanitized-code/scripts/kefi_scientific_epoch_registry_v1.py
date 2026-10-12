#!/usr/bin/env python3
"""Append-only hash-chained epoch registry with fail-closed eligibility."""
from __future__ import annotations
import argparse, hashlib, json, os, time
from pathlib import Path
SCHEMA="KEFI_SCIENTIFIC_EPOCH_REGISTRY_V1"
KINDS={"HISTORICAL_DEVELOPMENT","H1_OPENED","H2_QUARANTINED","OPERATIONAL","FUTURE_UNOPENED"}
STATUSES={"DEVELOPMENT_ONLY","QUARANTINED","OPERATIONAL_ONLY","UNOPENED"}
def canonical(v): return (json.dumps(v,sort_keys=True,separators=(",", ":"))+"\n").encode()
def digest(v): return hashlib.sha256(canonical(v)).hexdigest()
def load(p): return [] if not p.exists() else [json.loads(x) for x in p.read_text().splitlines() if x]
def path(root,e): return root/"records"/f"{e}.json"
def verify(root):
 previous=""
 for row in load(root/"epochs.jsonl"):
  claimed=row.pop("record_sha256")
  disk=json.loads(path(root,row["epoch_id"]).read_text()); disk_claimed=disk.pop("record_sha256")
  if row.get("previous_record_sha256")!=previous or digest(row)!=claimed or disk_claimed!=claimed or digest(disk)!=claimed: raise RuntimeError("FAIL_CLOSED: epoch registry hash chain mismatch")
  previous=claimed
def lock(root):
 deadline=time.monotonic()+10; p=root/".append.lock"
 while True:
  try: return os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
  except FileExistsError:
   if time.monotonic()>=deadline: raise RuntimeError("FAIL_CLOSED: epoch registry append lock unavailable")
   time.sleep(.05)
def append(root,payload):
 root.mkdir(parents=True,exist_ok=True); fd=lock(root)
 try:
  verify(root); rows=load(root/"epochs.jsonl")
  if any(x["epoch_id"]==payload["epoch_id"] for x in rows): raise RuntimeError("FAIL_CLOSED: epoch already exists")
  record={"schema":SCHEMA,**payload,"previous_record_sha256":rows[-1]["record_sha256"] if rows else ""}; record["record_sha256"]=digest(record)
  pending=root/"pending"/f"{payload['epoch_id']}.json"; pending.parent.mkdir(parents=True,exist_ok=True)
  with pending.open("wb") as h:h.write(canonical(record));h.flush();os.fsync(h.fileno())
  target=path(root,payload["epoch_id"]);target.parent.mkdir(parents=True,exist_ok=True)
  h=os.fdopen(os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o444),"wb");h.write(canonical(record));h.flush();os.fsync(h.fileno());h.close()
  with (root/"epochs.jsonl").open("ab") as h:h.write(canonical(record));h.flush();os.fsync(h.fileno())
  pending.unlink();return record
 finally: os.close(fd);(root/".append.lock").unlink(missing_ok=True)
def eligible(root,at,boundary):
 verify(root)
 if at<boundary: raise RuntimeError("INELIGIBLE_PRE_BOUNDARY")
 for r in load(root/"epochs.jsonl"):
  if r["start_utc"]<=at<=(r.get("end_utc") or "9999-12-31T23:59:59Z") and r["status"]!="UNOPENED": raise RuntimeError(f"INELIGIBLE_EPOCH_{r['kind']}")
def main():
 p=argparse.ArgumentParser();p.add_argument("--registry",type=Path,required=True);p.add_argument("--verify",action="store_true");p.add_argument("--eligible-at-utc");p.add_argument("--boundary-utc");p.add_argument("--epoch-id");p.add_argument("--kind",choices=sorted(KINDS));p.add_argument("--status",choices=sorted(STATUSES));p.add_argument("--start-utc");p.add_argument("--end-utc",default="");p.add_argument("--reason",default="");a=p.parse_args()
 if a.verify: verify(a.registry);print("VERIFIED");return
 if a.eligible_at_utc or a.boundary_utc: eligible(a.registry,a.eligible_at_utc,a.boundary_utc);print("ELIGIBLE");return
 if not all((a.epoch_id,a.kind,a.status,a.start_utc)):raise SystemExit("epoch fields required")
 print(json.dumps(append(a.registry,{"epoch_id":a.epoch_id,"kind":a.kind,"status":a.status,"start_utc":a.start_utc,"end_utc":a.end_utc,"reason":a.reason}),sort_keys=True))
if __name__=="__main__":main()
