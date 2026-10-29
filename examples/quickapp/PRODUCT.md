# Astrolink QuickApp

<!-- impeccable:product-schema 1 -->

## Platform

Vela watch QuickApp (.ux, firmware QuickJS); this platform is outside the skill's web/iOS/Android presets.

## Users

Developers validating two nearby watches with the native Astrolink module installed.

## Product Purpose

Expose scan, advertising, peer connection, binary send/receive, echo verification, disconnect and cleanup in one Chinese-language example.

## Operating Context

A narrow touchscreen watch, usually indoors during development. The example uses the native Feature binding directly. Two devices must share serviceId `chat`. Firmware and radio validation remain distinct from host tests.

## Capabilities and Constraints

Use the public Session/Link contract in `../../docs/ble-module-investigation.md`. No file IPC, Bluetooth substitute, external network dependency or mock transport in the shipped app. Show native errors and bound log/peer history. Release resources on app destruction.

## Brand Commitments

Astrolink is the product name. The parent-provided single-page wireframe fixes the hierarchy: status, scan/advertise, peers, Ping, boundary tests, disconnect and event results. Chinese interface copy; English code/comments.

## Assumptions

A black OLED ground and firmware system font suit the watch and keep the sample light. These are scoped practical defaults inferred under the parent's explicit instruction to proceed.
