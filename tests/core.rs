use astrolink::core::*;
use astrolink::protocol::*;

/// Links whose TX and RX queues together exhaust the shared message pool.
const FILLING: usize = POOL / (2 * QUEUE_DEPTH);
const _: () = assert!(POOL.is_multiple_of(2 * QUEUE_DEPTH));
const _: () = assert!(FILLING > 0 && FILLING + 2 <= 16);

fn peer(index: u8) -> Peer {
    Peer {
        address: [index, 2, 3, 4, 5, 6],
        address_type: 1,
    }
}

fn advertising_of(uuid: [u8; SERVICE_UUID_BYTES], data: &[u8], spsm: u8) -> Vec<u8> {
    let advertisement = Advertisement {
        uuid,
        spsm,
        data: AdvertisingData::new(data).unwrap(),
    };
    let mut bytes = [0; 31];
    let len = advertisement.encode(&mut bytes).unwrap();
    bytes[..len].to_vec()
}

fn advertising(service: &str, data: &[u8], spsm: u8) -> Vec<u8> {
    advertising_of(ServiceId::new(service).unwrap().uuid(), data, spsm)
}

fn hello_with(uuid: [u8; SERVICE_UUID_BYTES]) -> Vec<u8> {
    let mut bytes = [0; MAX_HELLO_BYTES];
    let len = encode_hello(uuid, &mut bytes);
    bytes[..len].to_vec()
}

fn hello_of(service: &str) -> Vec<u8> {
    hello_with(ServiceId::new(service).unwrap().uuid())
}

fn discover<const N: usize>(engine: &mut Engine<N>, session: SessionId, remote: Peer) {
    if engine.session(session).unwrap().scan == ResourceState::Idle {
        engine.begin_scan(session, false).unwrap();
        engine.scan_started(session, Ok(())).unwrap();
    }
    let uuid = engine.session(session).unwrap().uuid;
    engine.advertisement(session, remote, -40, &advertising_of(uuid, &[7], 0x80));
    // Reconnecting to a cached peer may suppress a duplicate advertisement event.
    assert!(engine.session(session).unwrap().peer(remote).is_some());
}

fn open<const N: usize>(
    engine: &mut Engine<N>,
    session: SessionId,
    remote: Peer,
    cid: u16,
) -> LinkId {
    discover(engine, session, remote);
    let (link, fresh) = engine.connect(session, remote, 100).unwrap();
    assert!(fresh);
    engine.acl_connected(link).unwrap();
    engine.channel_opened(link, cid, 512).unwrap();
    let mut hello = [0; MAX_HELLO_BYTES];
    let len = encode_hello(engine.session(session).unwrap().uuid, &mut hello);
    engine.receive(link, &hello[..len]).unwrap();
    engine.transmission_submitted(link, None).unwrap();
    assert_eq!(
        engine.take_event(),
        Some(Event::Connected {
            link,
            session,
            inbound: false
        })
    );
    link
}

fn saturated_pool() -> (Engine<16>, SessionId, [LinkId; 16]) {
    let mut engine = Engine::<16>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let links = std::array::from_fn(|i| open(&mut engine, session, peer(i as u8), 0x40 + i as u16));
    for (i, &link) in links[..FILLING].iter().enumerate() {
        for n in 0..QUEUE_DEPTH {
            engine.send(link, &[i as u8, n as u8]).unwrap();
            engine.receive(link, &[0x80 | i as u8, n as u8]).unwrap();
        }
    }
    (engine, session, links)
}

#[test]
fn standard_advertisement_roundtrips_at_legacy_limit() {
    let uuid = ServiceId::new("k7m2p9").unwrap().uuid();
    let data = [1, 2, 3, 4, 5, 6, 7, 8];
    let bytes = advertising("k7m2p9", &data, 0xff);
    assert_eq!(bytes.len(), 31);
    assert_eq!(&bytes[..5], &[2, 1, 2, 27, 0x21]);
    let parsed = Advertisement::decode(&bytes, uuid).unwrap();
    assert_eq!(parsed.uuid, uuid);
    assert_eq!(parsed.data.as_bytes(), &data);
    assert_eq!(parsed.spsm, 0xff);
    for end in 1..bytes.len() {
        assert!(Advertisement::decode(&bytes[..end], uuid).is_none());
    }
    // A peer with a different name is a different identity and is not parsed.
    assert!(Advertisement::decode(&bytes, ServiceId::new("other").unwrap().uuid()).is_none());
}

#[test]
fn advertisement_rejects_bad_version_psm_duplication_and_bounds() {
    let uuid = ServiceId::new("chat").unwrap().uuid();
    let valid = advertising("chat", &[0; MAX_ADVERTISING_DATA_BYTES], 0x80);
    for (offset, value) in [(5, 0), (21, 1), (22, 0x7f)] {
        let mut bad = valid.clone();
        bad[offset] = value;
        assert!(Advertisement::decode(&bad, uuid).is_none(), "offset {offset}");
    }
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid[3..]);
    assert!(Advertisement::decode(&duplicate, uuid).is_none());

    assert_eq!(ServiceId::new(""), Err(Error::InvalidArgument));
    let longest = "a".repeat(MAX_SERVICE_ID_BYTES);
    assert!(ServiceId::new(&longest).is_ok());
    assert_eq!(
        ServiceId::new(&"a".repeat(MAX_SERVICE_ID_BYTES + 1)),
        Err(Error::InvalidArgument)
    );
    // The name is local, so multi-byte text is bounded by bytes, not characters.
    assert!(ServiceId::new("中文服务名").is_ok());
    assert_eq!(
        AdvertisingData::new(&[0; MAX_ADVERTISING_DATA_BYTES + 1]),
        Err(Error::InvalidArgument)
    );

    // Identity is deterministic and name-dependent.
    assert_eq!(uuid, ServiceId::new("chat").unwrap().uuid());
    assert_ne!(uuid, ServiceId::new("chat ").unwrap().uuid());
}

#[test]
fn opaque_peer_identity_preserves_address_type() {
    for kind in 0..=3 {
        let remote = Peer {
            address_type: kind,
            ..peer(0xff)
        };
        let encoded = remote.encode();
        assert_eq!(
            Peer::decode(std::str::from_utf8(&encoded).unwrap()),
            Ok(remote)
        );
    }
    for value in ["", "1", "ff010203040506", "01zz0203040506"] {
        assert_eq!(Peer::decode(value), Err(Error::InvalidArgument));
    }
}

#[test]
fn scan_filters_service_and_deduplicates_complete_user_data() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    engine.begin_scan(session, false).unwrap();
    let first = advertising("chat", &[1], 0x80);
    assert!(engine
        .advertisement(session, peer(1), -40, &first)
        .is_none());
    engine.scan_started(session, Ok(())).unwrap();
    assert!(engine
        .advertisement(session, peer(1), -40, &advertising("other", &[1], 0x80))
        .is_none());
    assert!(engine
        .advertisement(session, peer(1), -40, &first)
        .is_some());
    assert!(engine
        .advertisement(session, peer(1), -90, &first)
        .is_none());
    assert_eq!(
        engine.session(session).unwrap().peer(peer(1)).unwrap().rssi,
        -90
    );
    assert!(engine
        .advertisement(session, peer(1), -50, &advertising("chat", &[2], 0x80))
        .is_some());
    assert!(engine
        .advertisement(session, peer(1), -50, &advertising("chat", &[2], 0x81))
        .is_some());
    assert!(engine.begin_stop_scan(session).unwrap());
    assert!(!engine.begin_stop_scan(session).unwrap());
    engine.scan_stopped(session).unwrap();
    assert!(!engine.begin_stop_scan(session).unwrap());
    engine.begin_scan(session, true).unwrap();
    engine.scan_started(session, Ok(())).unwrap();
    for _ in 0..3 {
        assert!(engine
            .advertisement(session, peer(1), -40, &first)
            .is_some());
    }
}

#[test]
fn duplicate_session_and_resource_start_return_busy() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    assert_eq!(engine.create_session(2, "chat"), Err(Error::Busy));
    engine.begin_scan(session, false).unwrap();
    assert_eq!(engine.begin_scan(session, true), Err(Error::Busy));
    assert_eq!(
        engine.scan_started(session, Err(Error::Backend(-7))),
        Err(Error::Backend(-7))
    );
    assert_eq!(engine.session(session).unwrap().scan, ResourceState::Idle);
    engine.begin_advertising(session).unwrap();
    assert_eq!(engine.begin_advertising(session), Err(Error::Busy));
    engine.advertising_started(session, Ok(())).unwrap();
    assert!(engine.begin_stop_advertising(session).unwrap());
    assert!(!engine.begin_stop_advertising(session).unwrap());
    engine.advertising_stopped(session).unwrap();
    assert!(!engine.begin_stop_advertising(session).unwrap());
}

#[test]
fn both_hello_orders_complete_only_after_local_submission() {
    for remote_first in [true, false] {
        let mut engine = Engine::<1>::new();
        let session = engine.create_session(1, "chat").unwrap();
        discover(&mut engine, session, peer(1));
        let (id, _) = engine.connect(session, peer(1), 0).unwrap();
        assert_eq!(engine.connect(session, peer(1), 5), Ok((id, false)));
        engine.channel_opened(id, 0x40, 512).unwrap();
        let hello = hello_with(engine.session(session).unwrap().uuid);
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.message.as_bytes(), &hello);
        assert!(tx.send.is_none());
        if remote_first {
            engine.receive(id, &hello).unwrap();
        } else {
            engine.transmission_submitted(id, None).unwrap();
        }
        assert_eq!(engine.link(id).unwrap().state, LinkState::CheckingService);
        assert!(engine.take_event().is_none());
        if remote_first {
            engine.transmission_submitted(id, None).unwrap();
        } else {
            engine.receive(id, &hello).unwrap();
        }
        assert_eq!(engine.link(id).unwrap().state, LinkState::Connected);
        assert_eq!(
            engine.take_event(),
            Some(Event::Connected {
                link: id,
                session,
                inbound: false
            })
        );
        assert!(engine.next_transmission().is_none());
    }
}

#[test]
fn mismatch_and_small_remote_mtu_close_the_channel() {
    let mismatch = hello_of("nope");
    for (mtu, hello) in [(512, &mismatch[..]), (5, &b""[..])] {
        let mut engine = Engine::<1>::new();
        let session = engine.create_session(1, "chat").unwrap();
        discover(&mut engine, session, peer(1));
        let (id, _) = engine.connect(session, peer(1), 0).unwrap();
        if mtu == 5 {
            assert_eq!(
                engine.channel_opened(id, 0x40, mtu),
                Err(Error::InvalidArgument)
            );
        } else {
            engine.channel_opened(id, 0x40, mtu).unwrap();
            assert_eq!(engine.receive(id, hello), Err(Error::InvalidArgument));
        }
        assert_eq!(engine.link(id).unwrap().state, LinkState::Closed);
        assert_eq!(engine.next_disconnect(), Some((id, Some(0x40))));
    }
}

#[test]
fn one_send_is_one_sdu_and_respects_the_remote_receive_limit() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let id = open(&mut engine, session, peer(1), 0x40);
    for size in [0, 1, 511, 512] {
        let bytes: Vec<_> = (0..size)
            .map(|i| (i as u8).wrapping_add(size as u8))
            .collect();
        let send = engine.send(id, &bytes).unwrap();
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.send, Some(send));
        assert_eq!(tx.message.as_bytes(), bytes);
        // Looking at a blocked host transmission does not consume or complete it.
        let retry = engine.next_transmission().unwrap();
        assert_eq!(retry.send, Some(send));
        assert_eq!(retry.message.as_bytes(), bytes);
        engine.transmission_submitted(id, Some(send)).unwrap();
        assert!(engine.next_transmission().is_none());
        engine.receive(id, &bytes).unwrap();
        let received = engine.take_received(id).unwrap().unwrap();
        assert_eq!(received.as_bytes(), bytes);
        assert!(engine.take_received(id).unwrap().is_none());
        // Both values remain owned after their pool slots are released and reused.
        assert_eq!(tx.message.as_bytes(), bytes);
        assert_eq!(retry.message.as_bytes(), bytes);
    }
    assert_eq!(engine.send(id, &[0; 513]), Err(Error::InvalidArgument));
}

#[test]
fn local_receive_limit_is_independent_of_remote_receive_mtu() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    discover(&mut engine, session, peer(1));
    let (id, _) = engine.connect(session, peer(1), 0).unwrap();
    engine.channel_opened(id, 0x40, 23).unwrap();
    engine
        .receive(id, &hello_with(engine.session(session).unwrap().uuid))
        .unwrap();
    engine.transmission_submitted(id, None).unwrap();
    assert_eq!(engine.link(id).unwrap().max_message_bytes, 23);
    assert_eq!(engine.send(id, &[0; 24]), Err(Error::InvalidArgument));
    engine.receive(id, &[1; 512]).unwrap();
    assert_eq!(
        engine.take_received(id).unwrap().unwrap().as_bytes(),
        &[1; 512]
    );
}

#[test]
fn tx_full_returns_busy_and_preserves_order() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let id = open(&mut engine, session, peer(1), 0x40);
    let ids: Vec<_> = (0..QUEUE_DEPTH)
        .map(|i| engine.send(id, &[i as u8]).unwrap())
        .collect();
    assert_eq!(engine.send(id, &[99]), Err(Error::Busy));
    for (i, send) in ids.into_iter().enumerate() {
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.send, Some(send));
        assert_eq!(tx.message.as_bytes(), &[i as u8]);
        engine.transmission_submitted(id, Some(send)).unwrap();
    }
}

#[test]
fn two_links_have_independent_queues_and_a_blocked_link_does_not_starve_other() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let a = open(&mut engine, session, peer(1), 0x40);
    let b = open(&mut engine, session, peer(2), 0x41);
    engine.send(a, b"a").unwrap();
    let sb = engine.send(b, b"b").unwrap();
    let first = engine.next_transmission().unwrap();
    let second = engine.next_transmission().unwrap();
    assert_ne!(first.link, second.link);
    let tx_b = if first.link == b { first } else { second };
    assert_eq!(tx_b.message.as_bytes(), b"b");
    engine.transmission_submitted(b, Some(sb)).unwrap();
    engine.receive(a, b"incoming a").unwrap();
    engine.receive(b, b"incoming b").unwrap();
    assert_eq!(
        engine.take_received(b).unwrap().unwrap().as_bytes(),
        b"incoming b"
    );
    assert_eq!(
        engine.take_received(a).unwrap().unwrap().as_bytes(),
        b"incoming a"
    );
    engine.channel_closed(a, Error::Disconnected).unwrap();
    engine.receive(b, b"still connected").unwrap();
    assert_eq!(
        engine.take_received(b).unwrap().unwrap().as_bytes(),
        b"still connected"
    );
}

#[test]
fn rx_overflow_closes_only_its_link_without_silent_drop() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let a = open(&mut engine, session, peer(1), 0x40);
    let b = open(&mut engine, session, peer(2), 0x41);
    for _ in 0..QUEUE_DEPTH {
        engine.receive(a, b"queued before listener").unwrap();
    }
    assert_eq!(engine.receive(a, b"overflow"), Err(Error::ReceiveOverflow));
    assert_eq!(
        engine.take_event(),
        Some(Event::Disconnected {
            link: a,
            session,
            reason: Error::ReceiveOverflow
        })
    );
    assert_eq!(engine.next_disconnect(), Some((a, Some(0x40))));
    assert_eq!(engine.link(b).unwrap().state, LinkState::Connected);
    engine.receive(b, b"ok").unwrap();
}

#[test]
fn peer_can_send_before_local_hello_is_submitted() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    discover(&mut engine, session, peer(1));
    let (id, _) = engine.connect(session, peer(1), 0).unwrap();
    engine.channel_opened(id, 0x40, 512).unwrap();
    engine
        .receive(id, &hello_with(engine.session(session).unwrap().uuid))
        .unwrap();
    engine.receive(id, b"first SDU").unwrap();
    assert!(engine.take_received(id).is_err());
    engine.transmission_submitted(id, None).unwrap();
    assert_eq!(
        engine.take_received(id).unwrap().unwrap().as_bytes(),
        b"first SDU"
    );
}

#[test]
fn late_callbacks_and_old_handles_do_not_touch_reused_slots() {
    let mut engine = Engine::<1>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let old = open(&mut engine, session, peer(1), 0x40);
    engine.close_link(old, Error::Disconnected).unwrap();
    discover(&mut engine, session, peer(2));
    assert_eq!(engine.connect(session, peer(2), 0), Err(Error::Busy));
    engine.disconnect_submitted(old).unwrap();
    assert_eq!(engine.connect(session, peer(2), 0), Err(Error::Busy));
    engine.take_event().unwrap();
    let (new, _) = engine.connect(session, peer(2), 0).unwrap();
    assert_ne!(old, new);
    assert_eq!(engine.receive(old, b"late data"), Err(Error::Disconnected));
    assert_eq!(
        engine.channel_closed(old, Error::Backend(-1)),
        Err(Error::Disconnected)
    );
    assert_eq!(
        engine.channel_opened(old, 0x40, 512),
        Err(Error::Disconnected)
    );
    assert_eq!(engine.link(new).unwrap().state, LinkState::Connecting);
}

#[test]
fn timeout_and_context_cleanup_invalidate_pending_work() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(42, "chat").unwrap();
    discover(&mut engine, session, peer(1));
    let (id, _) = engine.connect(session, peer(1), 100).unwrap();
    engine.tick(100 + CONNECT_TIMEOUT_MS - 1);
    assert_eq!(engine.link(id).unwrap().state, LinkState::Connecting);
    engine.tick(100 + CONNECT_TIMEOUT_MS);
    assert_eq!(
        engine.take_event(),
        Some(Event::Disconnected {
            link: id,
            session,
            reason: Error::Timeout
        })
    );
    assert_eq!(engine.next_disconnect(), Some((id, None)));
    engine.begin_advertising(session).unwrap();
    let cleanup = engine.close_session(session).unwrap();
    assert_eq!(cleanup.scan, ResourceState::Active);
    assert_eq!(cleanup.advertising, ResourceState::Starting);
    assert!(engine.close_session(session).is_none());
    assert_eq!(
        engine.scan_started(session, Ok(())),
        Err(Error::Disconnected)
    );
    assert_eq!(
        engine.advertising_started(session, Ok(())),
        Err(Error::Disconnected)
    );
    let new = engine.create_session(43, "chat").unwrap();
    assert_ne!(new, session);
}

#[test]
fn stop_advertising_prevents_new_links_and_preserves_existing_link() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(1, "chat").unwrap();
    assert_eq!(engine.accept(session, peer(1), 0), Err(Error::Busy));
    engine.begin_advertising(session).unwrap();
    engine.advertising_started(session, Ok(())).unwrap();
    let id = engine.accept(session, peer(1), 0).unwrap();
    engine.channel_opened(id, 0x40, 512).unwrap();
    engine
        .receive(id, &hello_with(engine.session(session).unwrap().uuid))
        .unwrap();
    engine.transmission_submitted(id, None).unwrap();
    assert_eq!(
        engine.take_event(),
        Some(Event::Connected {
            link: id,
            session,
            inbound: true
        })
    );
    engine.begin_stop_advertising(session).unwrap();
    assert_eq!(engine.accept(session, peer(2), 0), Err(Error::Busy));
    engine.advertising_stopped(session).unwrap();
    assert!(engine.send(id, b"existing link").is_ok());
}

#[test]
fn incoming_coc_survives_controller_advertising_termination() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(1, "chat").unwrap();
    engine.begin_advertising(session).unwrap();
    engine.advertising_started(session, Ok(())).unwrap();
    // A legacy connectable set terminates when the ACL connects, before the
    // peer can send its CoC request. Listener intent remains until explicit stop.
    engine.advertising_stopped(session).unwrap();
    assert_eq!(
        engine.session(session).unwrap().advertising,
        ResourceState::Idle
    );
    let link = engine.accept(session, peer(1), 0).unwrap();
    assert!(engine.link(link).unwrap().inbound);
    // Stopping an already terminated set must still revoke listener admission.
    assert!(!engine.begin_stop_advertising(session).unwrap());
    assert_eq!(engine.accept(session, peer(2), 0), Err(Error::Busy));
    assert!(engine.link(link).is_ok());
    engine.close_session(session).unwrap();
    assert_eq!(engine.accept(session, peer(2), 0), Err(Error::Disconnected));
}

#[test]
fn failed_or_cancelled_advertising_never_admits_incoming_coc() {
    let mut engine = Engine::<2>::new();
    let session = engine.create_session(1, "chat").unwrap();
    engine.begin_advertising(session).unwrap();
    assert_eq!(engine.accept(session, peer(1), 0), Err(Error::Busy));
    assert_eq!(
        engine.advertising_started(session, Err(Error::Backend(7))),
        Err(Error::Backend(7))
    );
    engine.advertising_stopped(session).unwrap();
    assert_eq!(engine.accept(session, peer(1), 0), Err(Error::Busy));
    engine.begin_advertising(session).unwrap();
    engine.begin_stop_advertising(session).unwrap();
    assert_eq!(
        engine.advertising_started(session, Ok(())),
        Err(Error::Disconnected)
    );
    engine.advertising_stopped(session).unwrap();
    assert_eq!(engine.accept(session, peer(1), 0), Err(Error::Busy));
}

#[test]
fn exhausted_pool_returns_busy_until_submission_or_receive_releases_a_slot() {
    let (mut engine, _, links) = saturated_pool();
    let waiting = links[FILLING];
    assert_eq!(engine.send(waiting, &[0; 513]), Err(Error::InvalidArgument));
    assert_eq!(engine.send(waiting, b"waiting"), Err(Error::Busy));
    let tx = engine.next_transmission().unwrap();
    let retry = engine.next_transmission().unwrap();
    assert_eq!(tx.message.as_bytes(), &[0, 0]);
    assert_eq!(retry.message.as_bytes(), &[1, 0]);
    assert_eq!(engine.send(waiting, b"waiting"), Err(Error::Busy));
    assert_eq!(
        engine.transmission_submitted(tx.link, Some(SendId(u32::MAX))),
        Err(Error::Disconnected)
    );
    assert_eq!(engine.send(waiting, b"waiting"), Err(Error::Busy));
    engine.transmission_submitted(tx.link, tx.send).unwrap();
    engine.send(waiting, b"reused TX slot").unwrap();
    assert_eq!(engine.send(waiting, b"still full"), Err(Error::Busy));
    let received = engine.take_received(links[0]).unwrap().unwrap();
    assert_eq!(received.as_bytes(), &[0x80, 0]);
    engine.receive(waiting, b"reused RX slot").unwrap();
    assert_eq!(engine.send(waiting, b"still full"), Err(Error::Busy));
    assert_eq!(
        engine.take_received(waiting).unwrap().unwrap().as_bytes(),
        b"reused RX slot"
    );
    assert_eq!(tx.message.as_bytes(), &[0, 0]);
    assert_eq!(received.as_bytes(), &[0x80, 0]);
}

#[test]
fn exhausted_pool_closes_only_the_receiving_link_with_receive_overflow() {
    let (mut engine, session, links) = saturated_pool();
    let overflowed = links[FILLING];
    assert_eq!(
        engine.receive(overflowed, b"overflow"),
        Err(Error::ReceiveOverflow)
    );
    assert_eq!(engine.link(overflowed).unwrap().state, LinkState::Closed);
    assert_eq!(
        engine.take_event(),
        Some(Event::Disconnected {
            link: overflowed,
            session,
            reason: Error::ReceiveOverflow,
        })
    );
    assert_eq!(engine.next_disconnect(), Some((overflowed, Some(0x45))));
    for &link in links.iter().filter(|&&link| link != overflowed) {
        assert_eq!(engine.link(link).unwrap().state, LinkState::Connected);
    }
    assert_eq!(
        engine.send(links[FILLING + 1], b"still full"),
        Err(Error::Busy)
    );
    engine.take_received(links[0]).unwrap().unwrap();
    engine.receive(links[FILLING + 1], b"survivor").unwrap();
    assert_eq!(
        engine
            .take_received(links[FILLING + 1])
            .unwrap()
            .unwrap()
            .as_bytes(),
        b"survivor"
    );
}

#[test]
fn closing_links_with_full_tx_and_rx_queues_reclaims_the_pool_for_100_cycles() {
    let mut engine = Engine::<{ FILLING + 2 }>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let survivor = open(&mut engine, session, peer(250), 0x50);
    for cycle in 0..100 {
        let links: [LinkId; FILLING] =
            std::array::from_fn(|i| open(&mut engine, session, peer(i as u8), 0x40 + i as u16));
        for &link in &links {
            for _ in 0..QUEUE_DEPTH {
                engine.send(link, &[cycle; 512]).unwrap();
                engine.receive(link, &[cycle; 512]).unwrap();
            }
        }
        assert_eq!(engine.send(survivor, b"full"), Err(Error::Busy));
        for link in links {
            engine.close_link(link, Error::Disconnected).unwrap();
            // A duplicate close must not release any slot a second time.
            engine.close_link(link, Error::Disconnected).unwrap();
            engine.disconnect_submitted(link).unwrap();
            assert_eq!(
                engine.take_event(),
                Some(Event::Disconnected {
                    link,
                    session,
                    reason: Error::Disconnected,
                })
            );
        }
        let send = engine.send(survivor, &[cycle]).unwrap();
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.link, survivor);
        assert_eq!(tx.message.as_bytes(), &[cycle]);
        engine.transmission_submitted(survivor, Some(send)).unwrap();
        engine.receive(survivor, &[cycle]).unwrap();
        assert_eq!(
            engine.take_received(survivor).unwrap().unwrap().as_bytes(),
            &[cycle]
        );
    }
}

#[test]
fn sixteen_link_entries_all_allocate_close_and_reuse() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<Link>();
    let mut engine = Engine::<16>::new();
    let session = engine.create_session(1, "chat").unwrap();
    let mut previous = [None; 16];
    for _ in 0..2 {
        let links: [LinkId; 16] =
            std::array::from_fn(|i| open(&mut engine, session, peer(i as u8), 0x40 + i as u16));
        for (i, &link) in links.iter().enumerate() {
            assert_ne!(previous[i], Some(link));
            engine.send(link, &[i as u8]).unwrap();
            engine.receive(link, &[0x80 | i as u8]).unwrap();
        }
        discover(&mut engine, session, peer(16));
        assert_eq!(engine.connect(session, peer(16), 0), Err(Error::Busy));
        for (i, &link) in links.iter().enumerate() {
            assert_eq!(
                engine.take_received(link).unwrap().unwrap().as_bytes(),
                &[0x80 | i as u8]
            );
            engine.channel_closed(link, Error::Disconnected).unwrap();
            assert_eq!(
                engine.take_event(),
                Some(Event::Disconnected {
                    link,
                    session,
                    reason: Error::Disconnected,
                })
            );
            previous[i] = Some(link);
        }
    }
}

#[test]
fn closing_a_session_releases_all_its_pool_slots() {
    let (mut engine, session, links) = saturated_pool();
    engine.close_session(session).unwrap();
    for link in links {
        engine.disconnect_submitted(link).unwrap();
        assert!(matches!(
            engine.take_event(),
            Some(Event::Disconnected { .. })
        ));
    }
    let new_session = engine.create_session(2, "next").unwrap();
    for i in (0..FILLING).map(|i| i as u8) {
        let link = open(&mut engine, new_session, peer(i), 0x60 + u16::from(i));
        for _ in 0..QUEUE_DEPTH {
            engine.send(link, b"reused after session close").unwrap();
            engine.receive(link, b"reused after session close").unwrap();
        }
    }
}

#[test]
fn oversized_receive_closes_its_link_and_reclaims_queued_messages() {
    let (mut engine, _, links) = saturated_pool();
    assert_eq!(
        engine.receive(links[0], &[0; 513]),
        Err(Error::ReceiveOverflow)
    );
    for _ in 0..QUEUE_DEPTH {
        engine.send(links[FILLING], b"reclaimed TX").unwrap();
        engine.receive(links[FILLING], b"reclaimed RX").unwrap();
    }
    assert_eq!(
        engine.send(links[FILLING + 1], b"full again"),
        Err(Error::Busy)
    );
}

#[test]
fn core_storage_is_bounded_for_device_and_multilink_configuration() {
    // Every engine reserves the same 40-message pool, even with a single Link.
    assert!(std::mem::size_of::<Engine<1>>() < 22 * 1024);
    assert!(std::mem::size_of::<Engine<2>>() < 22 * 1024);
    assert!(std::mem::size_of::<Engine<16>>() < 24 * 1024);
}

#[test]
fn identity_derivation_is_a_frozen_vector() {
    // Pinned so a later refactor cannot silently change every peer's identity.
    assert_eq!(
        ServiceId::new("chat").unwrap().uuid(),
        [
            0x69, 0x7f, 0x5b, 0x54, 0xac, 0x75, 0x72, 0x77, 0xb8, 0x06, 0xe9, 0x7b, 0x40, 0x61,
            0x45, 0x53
        ]
    );
    assert_eq!(
        ServiceId::new("chat2").unwrap().uuid(),
        [
            0x4a, 0xf5, 0xc2, 0x75, 0x95, 0x83, 0xd9, 0x4f, 0x70, 0x81, 0x4a, 0xa8, 0x37, 0xb0,
            0x5e, 0x5b
        ]
    );
}
