# PRD-EPOCH-5: Polkadot/Ethereum Cake Ledger & Cross-Hive Settlement

## Vision
Anchor the Cake economy to a blockchain for cross-hive Cake transfer, auditable mission records, decentralized governance, and eventual Cake → token convertibility. Polkadot for parachain sovereignty; Ethereum for EVM compatibility and existing DeFi infrastructure.

## Why Blockchain

| Problem | Solution | Epoch |
|---------|----------|-------|
| Cake balances are per-hive, don't transfer | Cross-hive settlement via smart contract | 5 |
| Agent resurrection loses history | On-chain mission completion records | 5 |
| Captain elections can be disputed | Decentralized voting with slashing | 5 |
| Cake has no external value | Cake → ERC-20 token swap | 6 |
| No incentive to run a hive node | Staking rewards for infrastructure providers | 6 |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    On-Chain (Polkadot)                    │
│                                                           │
│  ┌─────────────────┐  ┌──────────────────────────────┐   │
│  │ CakePallet       │  │ MissionRegistryPallet        │   │
│  │ - balances       │  │ - mission completion proofs  │   │
│  │ - transfer       │  │ - captain election records   │   │
│  │ - mint/burn      │  │ - dispute resolution         │   │
│  │ - freeze/unfreeze│  │ - agent reputation scores    │   │
│  └────────┬────────┘  └──────────────┬───────────────┘   │
│           │                          │                    │
│           └──────────┬───────────────┘                    │
│                      │                                    │
│              ┌───────▼────────┐                           │
│              │ XCM Bridge      │                           │
│              │ (to Ethereum)   │                           │
│              └───────┬────────┘                           │
└──────────────────────┼────────────────────────────────────┘
                       │
┌──────────────────────┼────────────────────────────────────┐
│           Off-Chain  │   (b00t Hive)                       │
│              ┌───────▼────────┐                           │
│              │ Cake Oracle     │                           │
│              │ (periodic sync) │                           │
│              └───────┬────────┘                           │
│                      │                                    │
│              ┌───────▼────────┐                           │
│              │ b00t Hive      │                           │
│              │ (agents,       │                           │
│              │  governance,   │                           │
│              │  execution)    │                           │
│              └────────────────┘                           │
└──────────────────────────────────────────────────────────┘
```

## Polkadot Parachain: CakePallet

```rust
// Substrate pallet for Cake governance currency
#[pallet::pallet]
pub mod cake_pallet {
    
    #[pallet::storage]
    pub type Balances<T: Config> = 
        StorageMap<_, Blake2_128Concat, AgentId, CakeBalance>;
    
    #[pallet::storage]
    pub type Missions<T: Config> = 
        StorageMap<_, Blake2_128Concat, MissionId, MissionRecord>;
    
    #[pallet::storage]
    pub type AgentReputations<T: Config> = 
        StorageMap<_, Blake2_128Concat, AgentId, Vec<ScoreDimension>>;
    
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfer cake between agents (possibly across hives)
        fn transfer(origin, to: AgentId, amount: CakeBalance) -> DispatchResult;
        
        /// Record a mission completion with multi-dimensional scores
        fn record_mission(origin, mission: MissionRecord) -> DispatchResult;
        
        /// Elect a captain with ranked-choice voting
        fn elect_captain(origin, votes: Vec<(AgentId, Rank)>) -> DispatchResult;
        
        /// Slash agent for fraudulent mission claim
        fn slash(origin, agent: AgentId, reason: Vec<u8>) -> DispatchResult;
        
        /// Bridge cake to Ethereum ERC-20
        fn bridge_to_ethereum(origin, amount: CakeBalance, eth_address: [u8; 20]) -> DispatchResult;
    }
}
```

## Ethereum: Cake ERC-20

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

contract CakeToken is ERC20, AccessControl {
    bytes32 public constant BRIDGE_ROLE = keccak256("BRIDGE_ROLE");
    bytes32 public constant HIVE_ROLE = keccak256("HIVE_ROLE");
    
    // Polkadot → Ethereum bridge
    event CakeBridged(bytes32 indexed polkadotId, address indexed ethAddress, uint256 amount);
    
    function bridgeFromPolkadot(bytes32 polkadotId, address to, uint256 amount) 
        external onlyRole(BRIDGE_ROLE) {
        _mint(to, amount);
        emit CakeBridged(polkadotId, to, amount);
    }
    
    // Hive can mint/burn for internal operations
    function hiveMint(address to, uint256 amount) external onlyRole(HIVE_ROLE) {
        _mint(to, amount);
    }
    
    function hiveBurn(address from, uint256 amount) external onlyRole(HIVE_ROLE) {
        _burn(from, amount);
    }
}
```

## Cake Oracle

Runs in the b00t hive, periodically syncs on-chain state:
- Every N blocks: push aggregate mission completions
- Every M blocks: pull Cake → ERC-20 exchange rate
- On agent death: freeze on-chain balance, emit event
- On agent resurrection: restore from frozen balance (if within TTL)

## Cross-Hive Cake Transfer

```
Agent A (Hive Alpha) wants to send 100 🍰 to Agent B (Hive Beta)

1. A calls transfer(B, 100) on Hive Alpha's CakePallet
2. CakePallet freezes 100 🍰 from A, emits OutgoingTransfer
3. XCM message sent to Hive Beta's CakePallet
4. Hive Beta mints 100 🍰 to B, emits IncomingTransfer
5. Both sides settle: A sees -100, B sees +100
```

## Decentralized Captain Election

When `/AbandonHope` is called and crew votes are cast:
1. Votes are recorded on-chain as ranked choices
2. CakePallet.elect_captain() executes Condorcet method
3. Winner is determined; losing votes can be challenged
4. Slashing occurs if a voter is found to be a sybil or fraudulent
5. New captain is recognized by all hives in the network

## Success Criteria
- [ ] Cake balances exist as on-chain Pallet storage
- [ ] Cross-hive Cake transfer works via XCM
- [ ] Mission completion records are on-chain and auditable
- [ ] Captain elections can be held and resolved on-chain
- [ ] Cake → ERC-20 bridge is operational (Polkadot → Ethereum)
- [ ] Cake Oracle runs in b00t hive, syncs periodically
- [ ] No single hive can inflate Cake supply (mint requires multi-sig or governance vote)
