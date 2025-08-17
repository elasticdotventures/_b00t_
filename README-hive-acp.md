Agent Coordination Protocol (ACP) StepSync Protocol (v0.1)

A lightweight peer-to-peer coordination protocol for turn-based agent collaboration.
Lamport Timing

agent coordination system (step-based, peer-to-peer or brokered, language-agnostic).

1. Introduction

The Agent Coordination Protocol (ACP) defines a minimal mechanism for multiple autonomous or semi-autonomous agents to coordinate their actions in discrete steps. ACP is transport-agnostic and may be layered over NATS, WebSocket, QUIC, or peer-to-peer transports.

Agents communicate through broadcast STATUS, PROPOSE, and STEP messages. Synchronization is achieved via a Step Barrier mechanism.

2. Terminology

Agent: A participant in the system (human-in-the-loop, LLM-based, or software process).

Step: A discrete round of coordinated activity within an epoch.

Barrier: A condition that ensures all agents complete a step before the next begins.

Coordinator: (Optional) A designated agent or broker that validates step completion.

MUST, MUST NOT, SHOULD, SHOULD NOT, MAY: As defined in RFC 2119.

3. Message Types

All ACP messages MUST include:

step (integer, monotonically increasing)

agent_id (string, unique identifier)

type (string, one of STATUS, PROPOSE, STEP)

payload (arbitrary JSON object)

3.1 STATUS

Purpose: Convey current state or logs of an agent.

Semantics:

Agents MUST broadcast STATUS during a step.

STATUS MAY be repeated to reflect intermediate updates.

3.2 PROPOSE

Purpose: Suggest an action, plan, or mutation.

Semantics:

Agents SHOULD broadcast PROPOSE messages before step closure.

PROPOSE messages MAY be voted on or ignored, depending on coordination rules.

3.3 STEP

Purpose: Mark the completion of a step.

Semantics:

An agent MUST broadcast a STEP message when it is ready to advance.

A step is considered complete when all participating agents have broadcast STEP, OR a timeout has occurred.

4. Step Barrier

Each agent MUST locally track the current step.

An agent MUST NOT advance to step + 1 until:

It has broadcast its STEP message for the current step, AND

It has received STEP (or timeout) from all peers.

If a timeout occurs, the agent MAY advance regardless of missing STEP messages.

5. Transport

ACP MAY be implemented over NATS subjects, P2P multicast, or any reliable messaging bus.

Messages MUST be encoded in UTF-8 JSON.

Subjects or topics MUST include the step number to prevent cross-step message bleed.

Example subject for NATS:

acp.{step}.{agent_id}.{type}

6. Example Flow (3 Agents, NATS Transport)

Agents enter Step 1.

Each broadcasts STATUS logs.

One broadcasts a PROPOSE action.

All broadcast STEP(1).

Upon receiving all STEP(1), they increment local step → Step 2.

7. Security Considerations

Agents MUST validate agent_id uniqueness.

Agents SHOULD ignore messages with stale or future step values.

If operating in untrusted environments, ACP SHOULD be layered with transport-level authentication and encryption (e.g., mTLS over NATS).


Primitives

Agent — participant with unique id.

Step — discrete unit of coordination. All agents collect messages until step close.

Message — JSON object, broadcast to all peers:

{
  "type": "<announce|data|step|ack>",
  "agent": "<id>",
  "step": <int>,
  "payload": { ... }
}

Message Types

announce

Purpose: join the network.

Payload: { "capabilities": [...], "version": "0.1" }

data

Purpose: arbitrary domain message (logs, edits, votes).

Payload: user-defined.

step

Purpose: signal close of step.

Payload: { "step": <int> }

Rule: only one step per agent per step cycle.

ack

Purpose: confirm receipt of step.

Payload: { "step": <int>, "from": "<id>" }

Step Synchronization

Agents broadcast data messages asynchronously, tagged with step.

When an agent is ready, it broadcasts a step message.

Peers respond with ack for that step.

When an agent has received step + ack from all known peers (or timeout), the step is considered closed.

Next step begins (step = step + 1).

Timing Model

Steps advance only when all agents either:

broadcasted step + received all ack, or

timeout expires → force advance.

Example Flow

Agent A, B, C at step=5:

A → all: data(step=5, "log update")
B → all: data(step=5, "frontend patch")
C → all: data(step=5, "backend result")

A → all: step(5)
B → all: step(5)
C → all: step(5)

All → all: ack(5)
--> Step 5 closed, advance to Step 6

Constraints

Transport: NATS, WebRTC, QUIC, TCP — agnostic.

Ordering: per-agent FIFO required.

State: each agent maintains known_agents, current_step, received.

This gives a peer-to-peer, minimal, language-portable protocol.


```mermaid
sequenceDiagram
    participant A1 as Agent A1 (frontend)
    participant A2 as Agent A2 (backend)
    participant A3 as Agent A3 (validator)

    Note over A1,A2,A3: Starting at step=0

    A1->>A2: step.propose {step:1}
    A1->>A3: step.propose {step:1}
    A2->>A1: step.commit {step:1}
    A2->>A3: step.commit {step:1}
    A3->>A1: step.commit {step:1}
    A3->>A2: step.commit {step:1}
    Note over A1,A2,A3: Step 1 committed<br/>All agents act on buffered messages

    A2->>A1: step.propose {step:2}
    A2->>A3: step.propose {step:2}
    A1->>A2: step.commit {step:2}
    A1->>A3: step.commit {step:2}
    A3->>A1: step.commit {step:2}
    A3->>A2: step.commit {step:2}
    Note over A1,A2,A3: Step 2 committed<br/>All agents act on buffered messages

    A3->>A1: step.propose {step:3}
    A3->>A2: step.propose {step:3}
    A1->>A2: step.commit {step:3}
    A1->>A3: step.commit {step:3}
    A2->>A1: step.commit {step:3}
    A2->>A3: step.commit {step:3}
    Note over A1,A2,A3: Step 3 committed<br/>All agents act on buffered messages
```