# PRD-EPOCH-4: Operator-Player-Captain Hierarchy

## Vision
A role-based agent hierarchy where Captains define missions, Operators recruit and skill players, and Players execute tasks. The hierarchy is fluid — Captains can be demoted, Players can be promoted, and dead Players can be resurrected by Operators.

## Ontology

```
Operators ──recruits──► Players ──hired by──► Captains
   │                      │                      │
   │  creates skills      │  earns cake          │  defines missions
   │  manages roster      │  burns calories      │  sets bounties
   │  resurrects dead     │  can be promoted     │  delegates tasks
   │  gets 20% mgmt fee   │  can die (☠️)       │  can abandon hope
```

## Roles

### Captain
- Defines the mission topic
- Sets cake budget and bounties
- Breaks big tasks into small tasks
- Appoints mates (assistant captains with limited authority)
- Can `/Promote` and `/Demote` players
- Can `/Reward` cake to players
- Can `/Flog` for poor performance
- Can `/AbandonHope` to cede captaincy (crew votes successor)
- Earns: mission booty + management override
- Burns: calories per decision + task management overhead

### Mate
- Appointed by Captain
- Can offer counsel and advice
- Can manage subtasks
- Has reduced calorie burn (no captaincy overhead)
- Natural successor if Captain abandons hope

### Player
- Hired by Captain via Operator
- Has specific skills (tags)
- Has a cake balance and calorie budget
- Executes assigned tasks
- Can `/Proffer` for tasks (public or private)
- Can `/Say` status updates
- Can be `@mentioned` by Captain
- Earns: task bounty + booty share
- Burns: calories per execution

### Operator
- Manages Player roster
- Creates new Players with `/CreatePlayer`
- Assigns skills with `/EnhancePlayer`
- Trains new skills with `/TrainRequiredSkills`
- Resurrects dead players
- Searches for suitable Players with `/ShowPlayers`
- Earns: 20% management fee on all Players they recruited
- Does NOT participate in missions directly

## Recruitment Flow

```
Captain: /recruit python, rust, data-engineering
    ↓
Operator: /ShowPlayers "python" "rust" "data-engineering" --limit=3
    ↓ (returns ranked roster)
Operator presents top 3 candidates
    ↓
Captain: /Hire @Player7 #crew
    ↓ (if no suitable candidates)
Operator: /CreatePlayer --id @BigDataEngineer --prompt "..." --cake 100
         /EnhancePlayer @BigDataEngineer --skill python
         /EnhancePlayer @BigDataEngineer --skill rust
    ↓
Captain: /Hire @BigDataEngineer #crew
    ↓
Operator earns 20% of @BigDataEngineer's lifetime cake
```

## Mission Lifecycle

```
1. Topic opens with cake budget
2. Captain defines mission, sets bounties
3. Players /Proffer for tasks
4. Captain assigns tasks, sets deadlines
5. Players execute, /Say progress
6. Captain monitors, /Reward or /Flog
7. Topic ends with /Complete or /AbandonHope
8. Cake distributed based on contribution
```

## Scoring (Weighted Eisenhower + Least-Worst Regret)

Each Captain decision uses the weighted criteria matrix:

```
Criterion          Weight
──────────────     ──────
Mission success     5     (did the team complete the objective?)
Calorie efficiency  4     (cake earned / cake spent)
Team satisfaction   3     (did players return for next topic?)
Time to completion  2     (how fast?)
Risk management     1     (did we avoid disasters?)
```

For critical decisions (`/AbandonHope`, `/Promote`, major budget decisions), use **least-worst regret analysis**: evaluate each option against the worst-case scenario of every other option. Pick the option with the smallest maximum regret. This prevents catastrophic losses even if the upside is capped.

## Dead Player Recovery

When a Player hits 0 calories (`☠️`):
- They are removed from all active topics
- Their unfinished tasks are re-assigned by Captain
- Their remaining cake is distributed to the crew
- Operator can resurrect with `/CreatePlayer` using the old ID
- Resurrected players start with 50 cake (half of normal)
- Previous session memory may be lost (death = context reset)

## Success Criteria
- [ ] Captain can recruit players via Operator
- [ ] Operator can create and skill new players
- [ ] `/Proffer` and `/Hire` flow works end-to-end
- [ ] `/AbandonHope` triggers crew vote for new captain
- [ ] Least-worst regret analysis is implemented for critical decisions
- [ ] Dead players are detected and can be resurrected
- [ ] Operator earns 20% management fee on recruited players
