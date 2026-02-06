# B00t Pilot/Tower Protocol - 10-10 Communication
See full tutorial at: https://github.com/elasticdotventures/b00t

## Quick Start - 3 Core Tutorials

### Tutorial 1: Session Init (5 min)
```bash
SESSION_ID="sess-$(date +%Y%m%d)-001"
b00t-cli redis set "session:${SESSION_ID}:context" '{"mission":"test"}' --expire 3600
b00t-cli redis publish "b00t:tower:control" '{"type":"session_init","session_id":"'${SESSION_ID}'"}'
```

### Tutorial 2: Pilot Registration (10 min)
```bash
AGENT_ID="ralph-001"
b00t-cli redis publish "b00t:tower:ground" '{"type":"register_request","agent_id":"'${AGENT_ID}'","skills":["prd-parsing"]}'
# Wait for register_ack...
b00t-cli redis publish "b00t:tower:ground" '{"type":"ready_signal","agent_id":"'${AGENT_ID}'","code":"10-10"}'
```

### Tutorial 3: Job Claim (15 min)
```bash
JOB_ID="job-$(uuidgen | tr -d '-' | head -c 12)"
b00t-cli redis set "job:${JOB_ID}:spec" '{"type":"bash","command":"echo test"}' --expire 3600
redis-cli SET "job:${JOB_ID}:claimed_by" "ralph-001" NX EX 300
# If OK, execute job
b00t-cli redis set "job:${JOB_ID}:status" "complete"
b00t-cli redis publish "b00t:tower:arrival" '{"type":"job_complete","job_id":"'${JOB_ID}'","code":"10-10"}'
```

## 10-10 Codes
- 10-4: Acknowledged
- 10-10: Excellent signal/complete
- 10-20: Status request
- 10-33: Emergency

## Channels
- b00t:tower:control - Job coordination
- b00t:tower:ground - Registration
- b00t:tower:departure - Job dispatch
- b00t:tower:arrival - Completion reports
- b00t:pilot:{id} - Direct pilot comms

📘 Full tutorial with 6 modules available in b00t documentation.
