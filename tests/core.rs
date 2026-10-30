use astrolink::core::*;
use astrolink::protocol::*;

fn peer(index: u8) -> Peer {
    Peer {
        address: [index, 2, 3, 4, 5, 6],
        address_type: 1,
    }
}

fn advertising(service: &str, data: &[u8], spsm: u8) -> Vec<u8> {
    let advertisement = Advertisement {
        service: ServiceId::new(service).unwrap(),
        spsm,
        data: AdvertisingData::new(data).unwrap(),
    };
    let mut bytes = [0; 31];
    let len = advertisement.encode(&mut bytes).unwrap();
    bytes[..len].to_vec()
}

fn discover<const N: usize>(engine: &mut Engine<N>, session: SessionId, remote: Peer) {
    if engine.session(session).unwrap().scan == ResourceState::Idle {
        engine.begin_scan(session, false).unwrap();
        engine.scan_started(session, Ok(())).unwrap();
    }
    let service = engine.session(session).unwrap().service.as_str().to_owned();
    engine
        .advertisement(session, remote, -40, &advertising(&service, &[7], 0x80))
        .unwrap();
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
    let len = encode_hello(engine.session(session).unwrap().service, &mut hello);
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

#[test]
fn standard_advertisement_roundtrips_at_legacy_limit() {
    let bytes = advertising("k7m2p9", &[0xab], 0xff);
    assert_eq!(bytes.len(), 31);
    assert_eq!(&bytes[..5], &[2, 1, 2, 27, 0x21]);
    let parsed = Advertisement::decode(&bytes).unwrap();
    assert_eq!(parsed.service.as_str(), "k7m2p9");
    assert_eq!(parsed.data.as_bytes(), &[0xab]);
    assert_eq!(parsed.spsm, 0xff);
    for end in 1..bytes.len() {
        assert!(Advertisement::decode(&bytes[..end]).is_none());
    }
}

#[test]
fn advertisement_rejects_foreign_uuid_bad_version_psm_and_truncated_utf8() {
    let valid = advertising("chat", &[0], 0x80);
    for (offset, value) in [(5, 0), (21, 2), (22, 0x7f), (23, 7), (24, 0xff)] {
        let mut bad = valid.clone();
        bad[offset] = value;
        assert!(Advertisement::decode(&bad).is_none(), "offset {offset}");
    }
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid[3..]);
    assert!(Advertisement::decode(&duplicate).is_none());
    assert_eq!(ServiceId::new(""), Err(Error::InvalidArgument));
    assert_eq!(ServiceId::new("1234567"), Err(Error::InvalidArgument));
    assert!(ServiceId::new("测试").is_ok());
    assert_eq!(ServiceId::new("测试a"), Err(Error::InvalidArgument));
    assert_eq!(AdvertisingData::new(&[0, 1]), Err(Error::InvalidArgument));
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
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.message.as_bytes(), b"\x01\x04chat");
        assert!(tx.send.is_none());
        if remote_first {
            engine.receive(id, b"\x01\x04chat").unwrap();
        } else {
            engine.transmission_submitted(id, None).unwrap();
        }
        assert_eq!(engine.link(id).unwrap().state, LinkState::CheckingService);
        assert!(engine.take_event().is_none());
        if remote_first {
            engine.transmission_submitted(id, None).unwrap();
        } else {
            engine.receive(id, b"\x01\x04chat").unwrap();
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
    for (mtu, hello) in [(512, &b"\x01\x04nope"[..]), (5, &b""[..])] {
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
        let bytes = vec![size as u8; size];
        let send = engine.send(id, &bytes).unwrap();
        let tx = engine.next_transmission().unwrap();
        assert_eq!(tx.send, Some(send));
        assert_eq!(tx.message.as_bytes(), bytes);
        // Looking at a blocked host transmission does not consume or complete it.
        let retry = engine.next_transmission().unwrap();
        assert_eq!(retry.send, Some(send));
        engine.transmission_submitted(id, Some(send)).unwrap();
        assert!(engine.next_transmission().is_none());
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
    engine.receive(id, b"\x01\x04chat").unwrap();
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
    engine.receive(id, b"\x01\x04chat").unwrap();
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
    engine.receive(id, b"\x01\x04chat").unwrap();
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
fn core_storage_is_bounded_for_device_and_multilink_configuration() {
    assert!(std::mem::size_of::<Engine<1>>() < 8 * 1024);
    assert!(std::mem::size_of::<Engine<2>>() < 13 * 1024);
}
