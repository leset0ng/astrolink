# QEMU LE controller extension: implementation decision and validation plan

Status on 2026-10-30: the isolated controller implementation is built and its 19 controller regressions pass. It implements genuine LE initiation, connection events and independent ACL routing, not Astrolink or L2CAP/CoC emulation. The existing BR/EDR companion link remains independent.

- `hw/misc/bes_btss_le.c`: per-host LE event masks, filter-accept lists and initiators; legacy/extended Create and Cancel; enabled-advertiser matching; independent LE handle 2; Connection Complete, Remote Features, Connection Update and Disconnect.
- Shared ACL, remote-version and reset paths dispatch by handle, preserving the Classic handle 1 security state.
- `tests/functional/test_bes_btss_le.py`: actual QEMU qtest mailbox and external H4 host endpoints, with no CPU/AP execution or L2CAP substitutions.
- Binary: `/tmp/astrolink-qemu-le/build/qemu-system-arm`, SHA-256 `99b67e01d4a4c19d2352461da54a43b16d194fea9cd19a893df3cc756195390b`.

Verification run against exact AP `../../mifw/analysis-q66-177/155/vela_ap.bin`:

```text
...................
Ran 19 tests in 0.118s
OK
```

The 19 tests cover both roles and event fields, pending/cancel ordering, late advertiser matching, directed/nonconnectable rejection, legacy/enhanced masks, random addresses, malformed parameters, extended creation, filter-accept lists, ACL fragmentation and completion handles, LE disconnect/reconnect, Classic preservation and reset. These are controller-level results. They do not yet prove firmware Bluelet CoC or physical-device radio behavior.

The decision and implementation plan below remain as the design record.

## Inspected baseline and isolated ownership

- Source: `/Users/lesetong/develop/Mirage/External/qemu-miwear`, commit `ceac001066440c7ed1a4ff71b6e0923bc352cfbd`.
- Isolated worktree: `/tmp/astrolink-qemu-le`, branch `astrolink-le-controller`, prepared from that commit.
- Original checkout is unchanged. No existing Mirage device has been operated by this task.
- Original build configuration: `arm-softmmu`, TCG/system/VNC/pixman enabled, optional defaults off. Build script: `Mirage/Scripts/build-qemu.sh`.

## Decision

Keep the existing BR/EDR `BesBtssLink` and its pairing state intact. Add one independent LE ACL state and one pending initiator state per controller side. Allocate an LE handle distinct from existing Classic handle 1 (handle 2 is sufficient for the current two-controller air model). Dispatch shared ACL, completion, disconnect, reset and remote-version operations by handle.

This is the smallest change that can exercise real Bluelet LE GAP/L2CAP while the companion retains its Classic ACL. It does not add CoC behavior to the controller. L2CAP signaling and SDUs remain opaque ACL bytes routed between the two real host state machines.

Rejected alternatives:

- Reusing `s->link` for LE: overwrites the Classic phone/companion ACL and its security state.
- Reporting immediate success for every Create Connection: ignores address matching, advertiser connectability and cancellation; it is not an initiator model.
- Joining two H4 controller ports: those ports consume host Commands/ACL, not peer-controller events or air PDUs.
- Adding Astrolink/CoC awareness to QEMU: bypasses precisely the firmware code being tested.
- A cross-process air protocol in this change: larger scope than the existing two-controller model; first validate firmware versus a standard external LE host through the already supported H4 host endpoint.

## Files and existing boundaries

1. `include/hw/misc/bes_btss.h`
   - `BesBtssHost` currently records public/random addresses, advertising sets, and scan settings. It does not retain general or LE event masks, an accept list, privacy state, or initiating state.
   - `BesBtssState::link` is a singleton BR/EDR link; `page_timer` and `air_timer` are already owned by the device.
   - Add explicit LE state and initiator timer/state without reinterpreting the Classic security fields.
2. `hw/misc/bes_btss_link.c`
   - `le_record` stores advertiser parameters but discards direct-peer address/type.
   - `le_see` deliberately excludes directed advertisements; `adv_enable` is the natural trigger for matching a pending initiator after advertiser enable.
   - `btss_link_command` does not handle 0x200D, 0x200E or 0x2043.
   - `link_disconnect`, `link_handle_is`, `btss_link_acl`, `num_completed`, `link_remote_info`, `btss_link_host_reset`, and `link_lost` all assume Classic `s->link`.
   - `radio_process` already parses full host H4 commands and ACL and applies receive-queue capacity/air pacing; retain that path.
3. `hw/misc/bes_btss.c`
   - `hci_uses_cmd_status` lists LE create/features/update commands but generic handling emits only successful Command Status.
   - Read Local Supported Features advertises LE capabilities broadly. Host setup may therefore request remote LE features or connection updates after completion; those procedures need real completion events or explicit unsupported errors.

## Ordered implementation steps and acceptance

1. Add state and handle helpers.
   - LE link stores up/down, central side, handle, actual connected address/type for each side, interval, latency, supervision timeout and selected PHY.
   - Per-side initiator stores pending flag, peer filter/address/type, own address/type, and validated parameters, with completion attempted asynchronously on the QEMU virtual clock.
   - Store HCI general Event Mask and LE Event Mask with specification reset defaults. Do not silently claim resolving-list/privacy support that is not implemented.
   - Acceptance: existing Classic paths use the original state unchanged; handles 1 and 2 select independent transport state.
2. Parse and validate the Create commands before sending successful Command Status.
   - Legacy 0x200D has exactly 25 parameter bytes: scan interval/window, filter policy, peer address type/address, own address type, connection interval min/max, latency, supervision timeout, min/max CE length.
   - Extended 0x2043 has a ten-byte prefix (filter policy, own type, peer type/address, initiating PHY mask), then one 16-byte parameter block for each selected PHY, in PHY order. Reject empty/reserved masks and unimplemented selected PHYs explicitly.
   - Validate scan window <= interval; connection interval/latency/supervision ranges and their required timeout relationship; use validated values in completion events. Reject random local addressing until the appropriate random address has been set.
   - Filter-policy accept-list operation must either implement the standard accept list (0x2010/11/12 and membership matching) or return an explicit unsupported error. Never substitute address matching while ignoring the filter policy.
   - Acceptance: malformed and unsupported requests never leave a pending procedure; second initiate request returns Command Disallowed; successful requests remain pending if no matching advertiser exists.
3. Resolve initiation against actual advertiser state.
   - Match public/random address and type; for extended sets, use their set random address, not global random address.
   - Match only enabled connectable sets. Directed advertising additionally requires matching the intended initiator address/type, recorded from advertising parameters.
   - On a match, establish the LE link with opposite central/peripheral roles, stop initiating, disable only the advertising set used for connection, and emit the proper LE connection event to each side.
   - A legacy advertising procedure stops at connection. For an extended advertising command, also emit LE Advertising Set Terminated when that event is enabled, with the actual advertising handle and connection handle.
   - Acceptance: wrong address/type, nonconnectable advertisements and directed-peer mismatch do not connect; enabling the matching set after initiation completes the pending connection; another enabled set remains unchanged.
4. Implement cancellation and event-mask selection.
   - 0x200E has no parameters. Cancel an active procedure with successful Command Complete, followed by LE connection-complete status Unknown Connection Identifier (0x02) for that initiator. No pending procedure returns Command Disallowed.
   - Completion event selection is per receiving host: LE Enhanced Connection Complete (subevent 0x0A) when mask bit9 enabled; otherwise legacy subevent 0x01 when bit0 enabled. When both are enabled, send only Enhanced.
   - Enhanced event includes zero RPAs for nonprivacy public/random connections; do not invent resolved identity addresses.
   - Acceptance: exact event lengths and fields, cancel ordering, no second completion after a canceled timer, and no unsolicited legacy event when Enhanced was selected.
5. Route generic link operations by handle.
   - ACL and Number Of Completed Packets must use the submitted valid handle, retaining packet-boundary flags and existing fragmentation/pacing/backpressure.
   - Disconnect valid LE handle reports local-host termination to issuer and the requested valid reason to peer, tears down only LE, and permits later LE reconnect; unknown handles cannot close Classic.
   - Reset and external-host disconnect terminate all affected links independently. Pending initiations are canceled without future success callbacks. Reset masks/address state to documented defaults.
   - Universal Read Remote Version Information must recognize LE handles. Implement LE Read Remote Features (0x2016 with LE Remote Features Complete subevent0x04) and LE Connection Update (0x2013 with Connection Update Complete subevent0x03) if requested by the firmware; unsupported optional procedures return explicit status instead of accepted requests that never finish.
   - Acceptance: simultaneous Classic/LE ACL exchanges keep payloads and completion handles separate; dropping LE leaves the companion Classic ACL live.
6. Add command-level regression tests against the actual controller code.
   - Exercise both controller sides with recorded emitted HCI events; do not replace `btss_link_command` or fabricate expected CoC callbacks.
   - Required cases: public connection both roles; random address/type match and mismatch; nonconnectable/directed mismatch; late advertiser enable; legacy/enhanced mask precedence; extended-create PHY layout/length rejection; cancel before match and late callback suppression; independent adv-set disable; valid/invalid disconnect; reset/peer loss; ACL both directions with PB flags; Number Of Completed Packets matching handle; existing Classic page/pair/link/ACL behavior.
   - Acceptance: every success is backed by the actual controller state, and the test checks event byte lengths/fields rather than only successful Command Status.
7. Build an isolated binary and integrate without mutating the live device concurrently.
   - Configure `/tmp/astrolink-qemu-le/build` using the source worktree and original feature set; set Homebrew pkg-config paths as in the Mirage build script.
   - Build `qemu-system-arm`, run controller tests, record binary SHA256 and commit in the worktree. The parent simulator owner then explicitly chooses the binary for a fresh isolated boot.
   - Acceptance: production firmware still boots and companion connects; Bluelet sees real LE Connection Complete before issuing real CoC code0x14/0x15 on signaling CID5. CoC data acceptance requires observing actual ACL signaling and SDU exchange, not just controller tests.

## Specification basis

Bluetooth Core 6.1, Vol4 PartE, accessed 2026-10-29:

https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Core-61/out/en/host-controller-interface/host-controller-interface-functional-specification.html

Relevant sections: LE Create Connection, LE Create Connection Cancel, LE Extended Create Connection, LE Connection Complete, LE Enhanced Connection Complete, LE Set Event Mask, LE Advertising Set Terminated.

Official source confirms: canceled LE initiation completes with Unknown Connection Identifier (0x02); if legacy and Enhanced completion events are both enabled, Enhanced alone is emitted; LE mask bit0 enables legacy completion and bit9 enables Enhanced. Precise field tables and reset masks must be checked directly while implementing, not inferred from generic successful-command behavior.

## Material limits

This two-controller virtual-air model validates host/controller protocol behavior, not RF timing, power, encryption security or two physical devices. Full independent two-QEMU/AP communication would require an additional genuine controller-air backend; it is not achieved by connecting the H4 host ports. The proposed first integration endpoint is the existing external standard HCI host, with real Bluelet in the band firmware on the opposite side.
