use std::{num::NonZeroU32, sync::Arc, time::Duration};

use infrastructure::{
    rate_limit::{BucketStore, Limited, MemoryBuckets, Rate},
    testing::ManualClock,
};

fn rate() -> Rate {
    Rate::new(NonZeroU32::new(2).unwrap(), Duration::from_hours(1))
}

fn store(clock: &ManualClock, max_keys: usize) -> BucketStore {
    BucketStore::Memory(MemoryBuckets::new(Arc::new(clock.clone()), max_keys))
}

async fn take(store: &BucketStore, key: &str) -> Result<(), Limited> {
    store.take("login_ip", key, rate()).await.unwrap()
}

#[tokio::test]
async fn buckets_refill_one_interval_at_a_time() {
    let clock = ManualClock::starting_now();
    let store = store(&clock, 100);
    assert_eq!(take(&store, "a").await, Ok(()));
    assert_eq!(take(&store, "a").await, Ok(()));
    assert_eq!(
        take(&store, "a").await,
        Err(Limited {
            retry_after: Duration::from_mins(30)
        })
    );

    clock.advance(time::Duration::minutes(29));
    assert_eq!(
        take(&store, "a").await,
        Err(Limited {
            retry_after: Duration::from_mins(1)
        })
    );
    clock.advance(time::Duration::minutes(1));
    assert_eq!(take(&store, "a").await, Ok(()));
    assert!(take(&store, "a").await.is_err());

    store.give_back("login_ip", "a", rate()).await.unwrap();
    assert_eq!(take(&store, "a").await, Ok(()));
}

#[tokio::test]
async fn refilled_buckets_are_forgotten() {
    let clock = ManualClock::starting_now();
    let store = store(&clock, 100);
    take(&store, "a").await.unwrap();
    take(&store, "b").await.unwrap();
    assert_eq!(store.retain_recent().await.unwrap(), 0);

    clock.advance(time::Duration::minutes(30));
    assert_eq!(store.retain_recent().await.unwrap(), 2);
    let BucketStore::Memory(buckets) = &store else {
        panic!("a memory store")
    };
    assert!(buckets.is_empty());
}

#[tokio::test]
async fn a_flood_of_keys_stays_within_the_cap_and_keeps_recent_buckets() {
    let clock = ManualClock::starting_now();
    let store = store(&clock, 128);
    let BucketStore::Memory(buckets) = &store else {
        panic!("a memory store")
    };

    take(&store, "flooder").await.unwrap();
    take(&store, "flooder").await.unwrap();
    clock.advance(time::Duration::minutes(5));
    for n in 0..5_000 {
        take(&store, &format!("2001:db8:{n:x}::/64")).await.unwrap();
    }
    assert!(buckets.len() <= 128, "{}", buckets.len());
    // Evicting the bucket closest to refilling took the flood keys, never the flooder's fuller one:
    // flooding does not buy a fresh budget.
    assert!(take(&store, "flooder").await.is_err());
}
