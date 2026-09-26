use domain::{note::NoteId, user::UserId};

#[test]
fn ids_are_time_ordered() {
    let first = UserId::generate();
    let second = UserId::generate();
    assert!(first < second);
    assert_eq!(first.as_uuid().get_version_num(), 7);
}

#[test]
fn ids_round_trip_through_strings() {
    let id = NoteId::generate();
    assert_eq!(NoteId::parse(&id.to_string()).unwrap(), id);
    assert_eq!(id.to_string().parse::<NoteId>().unwrap(), id);
    assert_eq!(format!("{id:?}"), format!("Id({id})"));
    assert_eq!(uuid::Uuid::from(id), id.as_uuid());
}

#[test]
fn malformed_ids_are_rejected() {
    assert_eq!(UserId::parse("123").unwrap_err().code(), "invalid_id");
}

#[test]
fn ids_made_from_a_clock_carry_its_time_and_keep_their_order() {
    use domain::user::UserId;
    use time::{Duration, macros::datetime};

    let at = datetime!(2030-01-01 00:00 UTC);
    let id = UserId::generate_at(at);
    let (seconds, _) = id.as_uuid().get_timestamp().unwrap().to_unix();
    assert_eq!(i64::try_from(seconds).unwrap(), at.unix_timestamp());
    assert_eq!(id.as_uuid().get_version_num(), 7);

    // A frozen test clock still yields ids in creation order, and a later time sorts after all of
    // them.
    let frozen: Vec<UserId> = (0..100).map(|_| UserId::generate_at(at)).collect();
    assert!(frozen.windows(2).all(|pair| pair[0] < pair[1]));
    let later = UserId::generate_at(at + Duration::milliseconds(1));
    assert!(frozen.iter().all(|id| *id < later));

    // Callers with their own clocks (parallel tests) interleave: each one's ids still come out in
    // order.
    let first = UserId::generate_at(at);
    let _other_clock = UserId::generate_at(at + Duration::days(1));
    let second = UserId::generate_at(at);
    assert!(first < second);
    assert!(second < later);
}
