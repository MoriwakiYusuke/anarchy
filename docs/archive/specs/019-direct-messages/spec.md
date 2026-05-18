# Feature Specification: Direct Messages (DM)

**Feature Branch**: `019-direct-messages`
**Created**: 2026-04-20
**Status**: Draft
**Input**: User description: "DM機能を実装したい"

## Clarifications

### Session 2026-04-20

- Q: How should DMs hide the recipient's identity on-chain to satisfy FR-003? → A: Route all DMs to stealth addresses using the same logic as the stealth-reward mechanism (TODO.md §3.5, `pallet-stealth`). Recipients scan for DMs addressed to stealth addresses they can decrypt; main account IDs are never used as on-chain DM recipients.
- Q: What forward-secrecy posture does MVP require? → A: Per-message freshness only — each DM is encrypted under a fresh sender-ephemeral × recipient-long-term-key DH. No Double Ratchet at MVP. Consequence: if a recipient's long-term private key is later compromised, past DMs encrypted under that key can be decrypted. This tradeoff is accepted in exchange for implementation simplicity and to keep multi-device sync tractable.
- Q: How is multi-device access supported? → A: Reuse the existing stealth-reward key management model (016-stealth-address): DM reception private keys live in session memory only, and are moved between devices via a password-encrypted backup file that the user exports/imports explicitly. Losing the backup password means losing access to past DMs. Same infrastructure as `packages/wasm-engine/src/stealth/backup.rs` and `keys.rs`.
- Q: Does the sender's main account appear on-chain for a DM send? → A: No. Senders MUST also use a stealth address to dispatch a DM. The sender pre-funds a stealth account from their main account (using the existing stealth-transfer path), and the DM extrinsic is submitted from that stealth account. Both participants are thus hidden on-chain. Sender authentication to the recipient is conveyed inside the encrypted payload (signature over the DM under the sender's long-term identity key), so the recipient can verify authorship after decryption without leaking it to third parties.
- Q: What traffic-analysis resistance is required at MVP? → A: Fixed-size padding on every DM payload is mandatory at MVP (eliminates size correlation). Dummy-traffic / cover-traffic is explicitly deferred to a later feature (tracked alongside the remainder of TODO.md §3.3). MVP accepts residual timing-correlation risk; this tradeoff is documented and revisited if/when dummy traffic ships.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Send a private message to another user (Priority: P1)

An Anarchy user (Alice) wants to have a private, one-to-one conversation with another Anarchy user (Bob) that cannot be read by anyone else — not even the storage nodes, the blockchain validators, or network observers. Alice composes a text message, addresses it to Bob's account, and sends it. Bob sees the message appear in his inbox the next time he opens the app.

**Why this priority**: Private messaging is the defining capability of a DM feature. Without send + receive, there is no product. This single story is the MVP: if only this works, users already have a usable private-communication tool.

**Independent Test**: With two user accounts on a running testnet, Alice composes a message from her client, submits it, and Bob retrieves and reads the plaintext from his client. A third account querying the same data must see only ciphertext. Passing this flow delivers the full core value.

**Acceptance Scenarios**:

1. **Given** Alice and Bob both have active Anarchy accounts and Bob has published a message-encryption key, **When** Alice sends a text DM addressed to Bob, **Then** Bob sees the plaintext message in his inbox after syncing.
2. **Given** Alice has sent a DM to Bob, **When** any party other than Alice or Bob attempts to read the stored content or on-chain record, **Then** they see only ciphertext with no readable body, subject, or sender/recipient mapping in the clear.
3. **Given** Alice has insufficient MORAL tokens to cover the send fee, **When** Alice attempts to send a DM, **Then** the send is rejected with a clear "insufficient balance" explanation before any data leaves her device.

---

### User Story 2 - Read and manage inbox (Priority: P2)

Bob opens his inbox and sees all DMs addressed to him, grouped by the counterparty and ordered with the most recent conversation first. He can open a conversation to view the full history with that person, and can hide a sender he does not want to hear from again (see User Story also: blocking).

**Why this priority**: Sending is useless without a usable inbox. This story turns the MVP from a transport primitive into a communication UI. It is P2 rather than P1 because Story 1 can technically be demonstrated via a minimal list view.

**Independent Test**: Seed Bob's account with several DMs from multiple senders, open the inbox, verify ordering and grouping, open a conversation with Alice, and verify the full chronological history renders. (Note: individual-message deletion is intentionally out of scope — DMs follow the post lifecycle and are not user-deletable. See FR-018.)

**Acceptance Scenarios**:

1. **Given** Bob has received DMs from three different users, **When** Bob opens his inbox, **Then** he sees exactly three conversation threads ordered by most recent activity.
2. **Given** Bob has an open conversation with Alice containing 20 messages, **When** Bob scrolls the conversation, **Then** all 20 messages render in chronological order without gaps or duplicates.
3. **Given** Bob blocks a sender (see FR-011), **When** Bob returns to his inbox, **Then** that sender's conversation is hidden from Bob's view on all of his devices, while the underlying messages are not removed from storage.

---

### User Story 3 - Reply within a conversation (Priority: P2)

Within an open conversation, Bob types a reply and sends it. Alice receives the reply in the same conversation thread on her side.

**Why this priority**: Two-way exchange is table-stakes for DM. It is grouped with inbox (P2) rather than P1 because Story 1 already proves the send path works in one direction; reply is the symmetric application of the same primitive.

**Independent Test**: Alice sends an initial DM to Bob; Bob replies from his conversation view; Alice sees the reply threaded under the same conversation.

**Acceptance Scenarios**:

1. **Given** Alice and Bob have an existing conversation, **When** Bob sends a reply, **Then** Alice sees the reply in the same conversation thread with the correct ordering.
2. **Given** Bob is offline when Alice sends a message, **When** Bob comes back online, **Then** Alice's message appears in Bob's conversation on his next sync.

---

### User Story 4 - Delivery and read status feedback (Priority: P3)

Alice can see whether her sent message has been delivered to Bob's device and whether Bob has opened it.

**Why this priority**: Nice-to-have confidence signal. It can be added after core send/receive/inbox works, and some users will prefer it disabled for privacy.

**Independent Test**: Alice sends a DM; status starts as "sent"; transitions to "delivered" when Bob's device retrieves it; transitions to "read" when Bob opens the conversation. Alice sees each transition.

**Acceptance Scenarios**:

1. **Given** Bob is offline, **When** Alice sends a DM, **Then** Alice sees status "sent" and not "delivered".
2. **Given** Bob's device has pulled the message but Bob hasn't opened it, **When** Alice checks, **Then** she sees "delivered" (if Bob has not disabled receipts).
3. **Given** Bob has disabled read receipts in his settings, **When** Bob reads Alice's DM, **Then** Alice does not see a "read" status.

---

### Edge Cases

- **Recipient has no published encryption key**: Sender cannot encrypt, so the send MUST be blocked with a message explaining that the recipient has not yet enabled DMs.
- **Recipient account does not exist**: Send MUST be rejected before any fee is charged.
- **Message exceeds maximum size**: Send MUST be rejected with a clear size-limit error.
- **Network partition mid-send**: Sender's client MUST retry and eventually either succeed or surface a clear failure; no MORAL is charged for a send that never commits.
- **Recipient uses multiple devices**: Each device must have the same DM reception key material (imported from the same password-encrypted backup, per FR-022). Devices that have imported the backup see the same conversation content; devices that have not imported cannot decrypt.
- **Sender or recipient loses their account keys**: Loss of keys means loss of past DM access — the system MUST NOT have a recovery mechanism that would compromise end-to-end secrecy.
- **Replay / duplicate delivery**: The same message MUST NOT be shown twice in the recipient's view even if the transport delivers it more than once.
- **Abuse / spam**: The economic cost per DM discourages mass messaging; recipients MUST also be able to block a sender so future messages from that sender are not shown.
- **Content exceeds protocol limits**: DMs inherit the post pipeline's fragment size and chunking rules (current post spec: 256KB per fragment, >1MB chunked). Attempts to send content that cannot be fragmented under these rules MUST be rejected client-side with a clear error.
- **Message garbage-collected by the popularity system**: When a DM has been GC'd (see FR-018), recipients MUST see a clear indication that historical content is no longer retrievable rather than silently empty messages or crashes.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to address a direct message to another Anarchy user, identified in the UI by that user's Anarchy account. At the protocol level, the sender's client MUST derive a stealth address from the recipient's published DM reception key (per FR-003 / FR-012) and submit the DM against that stealth address rather than the recipient's main account ID.
- **FR-002**: The system MUST enforce end-to-end confidentiality: only the sender's and recipient's devices can derive the plaintext of a DM. Storage nodes, validators, and network observers MUST see only ciphertext.
- **FR-003**: The system MUST hide the participants of a DM from third parties by routing every DM to a stealth address rather than the recipient's main account ID. The stealth-address scheme MUST be the same one used by the stealth-reward mechanism (shared `pallet-stealth` logic: ephemeral-key + recipient-public-key derivation, with a client-side scan to detect messages addressed to stealth addresses the recipient can decrypt). A third party observing the network or on-chain data MUST NOT be able to enumerate the set of accounts any given user has exchanged DMs with.
- **FR-004**: The system MUST authenticate DMs such that a recipient can verify the message was produced by the claimed sender account and has not been tampered with. Authentication MUST be carried inside the encrypted payload (e.g. a signature over the DM body by the sender's long-term identity key, bound to the recipient's stealth address and a freshness value). Sender-account information MUST NOT be visible on-chain or to third-party storage nodes.
- **FR-005**: Users MUST be required to pay a non-trivial fee in MORAL tokens to send a DM. The fee MUST scale with message size using the same base+per-byte shape used elsewhere in the protocol. Concrete values are fixed in [plan.md](plan.md) and [contracts/pallet-messaging-extrinsics.md](contracts/pallet-messaging-extrinsics.md) as `DmBaseCost` (1 MORAL) and `DmByteCost` (0.05 MORAL/byte) with the post-parity 80% storage pool / 10% stealth-reward pool / 10% burn split.
- **FR-006**: The system MUST reject a send attempt when the sender has insufficient balance, the recipient account does not exist, or the recipient has not published a DM-reception public key.
- **FR-007**: Users MUST be able to view their inbox organized as conversations, one thread per counterparty, ordered by most-recent-first.
- **FR-008**: Users MUST be able to open a conversation and view its full message history in chronological order. Continuity across devices MUST be achieved by the user importing the same password-encrypted key-backup file (see FR-022) on each device, not by server-side or protocol-level sync.
- **FR-009**: The system MUST deliver DMs reliably: if a send commits on-chain, every honest instance of the recipient's client that syncs **while the message is still retained by the storage substrate** MUST eventually present the message. Delivery reliability is qualified by FR-018: once the popularity-driven GC (Phase 3.4) removes a DM, clients that first sync after GC MUST surface the *Message garbage-collected* edge-case indicator instead of attempting to recover the body. In the MVP period before Phase 3.4, "retained for the lifetime of the storage substrate" applies, so this qualifier is latent.
- **FR-010**: The system MUST NOT expose a user-initiated deletion action for individual DMs or conversations. DM lifecycle follows FR-018 (post-parity lifecycle + popularity-driven GC). Users MAY hide a sender from their own view via the block action (FR-011), but this does not remove stored content.
- **FR-011**: Users MUST be able to block a sender. After blocking, messages from that sender MUST NOT be shown to the blocker on any of the blocker's devices, even if delivery still occurs at the protocol layer.
- **FR-012**: Before sending, the sender's client MUST verify that the recipient's published encryption key is bound to the recipient's account (so a hostile node cannot swap keys to mount a man-in-the-middle attack).
- **FR-013**: DMs MUST inherit the existing post content pipeline's size and fragmentation rules (fragment cap, chunking behavior). The system MUST NOT introduce a separate DM-specific size limit.
- **FR-014**: All cryptographic operations (key derivation, encryption, signing) MUST occur on the sender's or recipient's own device. No private key material may ever be transmitted.
- **FR-020**: Each DM MUST be encrypted with a per-message symmetric key derived from a fresh sender-side ephemeral keypair and the recipient's long-term DM reception public key (ECDH + KDF). The system MUST NOT introduce a ratcheting session protocol (e.g. Double Ratchet) at MVP. The specification explicitly accepts that compromise of a recipient's long-term private key retroactively exposes prior DMs encrypted under that key.
- **FR-021**: The sender-side ephemeral keypair used for each DM MUST be generated freshly for every message, MUST NOT be reused across messages, and MUST be deleted from the sender's device after the send completes.
- **FR-022**: DM reception private keys MUST reuse the existing stealth-reward key management model (016-stealth-address): keys are held in session memory only, are never persisted to browser storage, and are moved between devices exclusively via a user-exported, password-encrypted backup file. The DM feature MUST share the same backup/import code path as the stealth-reward implementation (`packages/wasm-engine/src/stealth/backup.rs`, `keys.rs`) rather than introducing a parallel scheme.
- **FR-023**: If a user loses both their key material and their backup (or forgets the backup password), the system MUST NOT provide any recovery path for past DMs. The loss MUST be surfaced to the user on first sign-in after the loss as a clear, non-alarming message indicating that past DMs are unrecoverable.
- **FR-024**: The sender's client MUST dispatch every DM extrinsic from a stealth account rather than from the sender's main account. Before sending, the client MUST transfer the send fee (plus any buffer) from the sender's main account to a freshly derived stealth account using the existing stealth-transfer path, and submit the DM extrinsic from that stealth account. The system MUST NOT leave the sender's main account ID as the observable submitter of a DM transaction.
- **FR-025**: The sender's UI MUST surface the stealth pre-funding step transparently so the user understands that a small MORAL transfer precedes each DM send. Pre-funding MUST reuse the existing stealth-transfer UX primitives rather than introducing a DM-specific funding flow.
- **FR-026**: Every encrypted DM payload submitted on-chain or to distributed storage MUST be padded to one of a small, fixed set of canonical sizes before encryption is finalized. All payloads of a given canonical size MUST be indistinguishable by length. The set of canonical sizes and the rounding rule are chosen in planning; the requirement is that size does not leak plaintext length beyond the coarse bucket.
- **FR-027**: Cover-traffic / dummy-DM generation is explicitly **out of scope** for this feature. It is acknowledged that without cover traffic, timing correlation between DM submission and recipient-side activity remains a residual side channel. A follow-up feature (tracked against the rest of TODO.md §3.3) will introduce dummy traffic; until then the residual risk is accepted and surfaced in user-facing privacy documentation.
- **FR-015**: Users MUST be able to enable DM reception by publishing a DM-reception public key associated with their account. Users MUST also be able to disable reception (revoke/replace the key); after disablement, new sends MUST be blocked as in FR-006.
- **FR-016a**: The system SHOULD provide per-message delivery and read status (sent / delivered / read) to the sender. *(See User Story 4 — this is a P3 capability.)*
- **FR-016b**: Recipients MUST be able to disable read-receipt reporting without disabling the ability to receive DMs. The opt-out setting MUST persist across the recipient's devices via the same key-backup file (FR-022).
- **FR-017**: DMs MUST support the same content model as posts — i.e. arbitrary byte payloads constrained only by the post pipeline's fragment/chunk rules. No DM-specific content-type restrictions are introduced. Encryption operates on the opaque payload, so the transport is content-agnostic.
- **FR-018**: DMs MUST follow the post lifecycle: no user-initiated deletion, and GC governed by the popularity / retention system currently tracked as Phase 3.4 ("投稿人気度システム"). Until Phase 3.4 lands, delivered DMs are treated as retained for the lifetime of the storage substrate. Once 3.4 is in effect, DMs are subject to the same popularity-threshold + grace-period + storage-node-deletion flow as posts; the concrete popularity signals for DMs (e.g. whether read-events count) are deferred to that specification.
- **FR-019**: The MVP MUST support only 1:1 conversations. Group conversations (N > 2 participants) are explicitly out of scope and will be addressed in a future feature if pursued.

### Key Entities *(include if feature involves data)*

- **Direct Message**: A single unit of private communication. Attributes: stealth-address recipient, sender authentication proof, encrypted body, authenticated timestamp, delivery state (sent / delivered / read). The on-chain record does NOT contain the recipient's main account ID (FR-003). Exists as ciphertext everywhere except on the two participants' devices. Payload is opaque bytes, mirroring the post content model (FR-017).
- **Conversation**: Logical grouping of all Direct Messages exchanged between exactly two accounts (FR-019). Conversations are derived state — they have no identity independent of their participants and messages.
- **DM Reception Key**: A public key a user publishes to signal "I can receive DMs" and to let others encrypt to them. Bound to the user's account.
- **Block List**: Per-user set of accounts whose incoming DMs will be hidden from that user's view. Does not affect stored content on the storage substrate.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can send a text DM — including the stealth pre-funding step (FR-024) — and have it confirmed as sent in under 15 seconds on a typical consumer connection when both parties are online.
- **SC-002**: When both sender and recipient are online, 99% of DMs are visible in the recipient's inbox within 60 seconds of the sender confirming the send.
- **SC-003**: A third party — including a storage node operator and a validator — attempting to read the body, sender, or recipient of any DM has no higher success rate than random guessing. Plaintext length leakage via ciphertext size MUST fall within the coarse bucket defined by FR-026; finer-grained length inference MUST be infeasible. Verified by external cryptographic review before GA.
- **SC-004**: A user with up to 1,000 conversations can open their inbox and see the conversation list in under 3 seconds on a typical consumer device, measured once the client's scan cursor has caught up to head. (Initial sync on a new device is separately bounded — see Assumptions — and excluded from this target.)
- **SC-005**: A user with up to 10,000 messages in a single conversation can open that conversation and scroll to the oldest message without loss, duplication, or reordering.
- **SC-006**: In a representative beta cohort, at least 80% of users who attempt to send their first DM complete it without needing support intervention.
- **SC-007**: Abuse rate — DMs reported as spam per 1,000 delivered messages — stays below an acceptable operational threshold defined with the trust-and-safety team before launch.

## Assumptions

- DMs are built on the same off-chain-content + on-chain-commitment pattern already used for posts (content stored via distributed storage, a cryptographic commitment anchored on-chain). This is the natural fit for the existing architecture; revisit in planning only if it conflicts with privacy goals.
- DMs share the post pipeline's content model, fragmentation rules, and lifecycle (Q1 / Q2 resolution). The storage substrate does not need to distinguish DM payloads from post payloads at rest.
- At MVP, conversation topology is strictly 1:1 (Q3 resolution). Group chat is a separate future feature.
- Recipient discovery reuses the existing account identity system; no new public identity namespace is introduced.
- The set of accounts that have published DM reception keys (meta-addresses) is publicly observable on-chain, since `DmReceptionKeys` storage is queryable by any node. This is consistent with FR-003: the privacy guarantee is pair-level (no observer can tell *who is corresponding with whom*), not receiver-set-level (the fact that an account accepts DMs is not secret). Users who need to hide even the willingness-to-receive signal are out of scope for MVP.
- **Initial scan is a one-time offline-tolerant operation.** A new device joining a long-established account must scan every block since the account's DM-key publication; this is an O(blocks × max-dispatches-per-block) RPC workload that can legitimately take minutes to hours on a stale account. SC-004 ("open inbox in <= 5 s") applies to the *post-initial-sync steady state*, once the client's persisted scan cursor is caught up. Initial scan progress MUST be surfaced in the UI (loading / progress indicator) and MUST NOT block the rest of the app.
- Economic anti-spam follows the same base-fee + per-byte model as post creation, tuned separately so DMs are not disproportionately expensive relative to a post.
- Users accept that losing their keys means losing access to past DMs — this is a consequence of the end-to-end guarantee and is consistent with the project's existing "no raw private keys for users" principle backed by WebAuthn/Secure Enclave.
- No forward secrecy at MVP (FR-020): the tradeoff is explicit and user-visible in documentation. If/when a ratcheting scheme is adopted later, it will be a separate feature with its own migration story.
- Network-layer anonymity (Tor/I2P) is inherited from the existing transport; no DM-specific network hardening is in scope unless planning surfaces a gap.
- Read receipts, when present, are opt-out by default (enabled), which matches common messenger UX. The setting is user-visible and reversible.
- The popularity-driven GC rules for DMs are not finalized in this spec — they are explicitly deferred to Phase 3.4. Until then, "delivered = retained" is the operating assumption (per FR-018).

## Dependencies

- **Account identity system**: DMs address recipients by existing Anarchy account identifiers. No changes expected; must be stable.
- **Distributed storage (pallet-storage + storage-node)**: Provides the off-chain ciphertext substrate. DM-sized payloads must be supported; planning must confirm fragment/size behavior.
- **MORAL token + balances**: Required for the send fee. Must exist at launch (already exists).
- **Client-side crypto engine (packages/wasm-engine)**: Encryption, key derivation, and signing must be implemented or extended here to preserve the client-side-only-crypto principle.
- **Transport anonymity (libp2p over Tor/I2P)**: Already in place; DM traffic inherits it.
- **Notification / inbox sync**: Whether this is a new client-side subsystem or an extension of an existing one is a planning decision.
- **Phase 3.4 popularity / retention system** *(future)*: FR-018 defers DM GC rules to this future work. The DM spec does not block on 3.4 — DMs can ship with "retained for the lifetime of the storage substrate" semantics, and the popularity-based GC is applied uniformly once 3.4 is live.
- **`pallet-stealth` (Phase 3.5)**: FR-003 reuses the stealth-address mechanism being introduced for reaction-mining rewards. DM and stealth-reward features MUST share the same derivation, verification, and scan logic. Timing: the stealth-reward work must land first or in parallel; DM is blocked on the shared primitive, not on 3.5's reaction-reward integration.
