use domain::pagination::{
    Cursor, DEFAULT_PAGE_SIZE, MAX_CURSOR_BYTES, MAX_PAGE_SIZE, NewestFirst, Page, PageRequest,
    PageSize, Sort,
};
use uuid::Uuid;

#[test]
fn page_size_defaults_and_bounds() {
    assert_eq!(PageSize::parse(None).unwrap().get(), DEFAULT_PAGE_SIZE);
    assert_eq!(PageSize::default().get(), DEFAULT_PAGE_SIZE);
    assert_eq!(PageSize::parse(Some(1)).unwrap().get(), 1);
    assert_eq!(
        PageSize::parse(Some(MAX_PAGE_SIZE)).unwrap().get(),
        MAX_PAGE_SIZE
    );
    assert_eq!(PageSize::parse(Some(0)).unwrap_err().code(), "out_of_range");
    assert_eq!(
        PageSize::parse(Some(MAX_PAGE_SIZE + 1)).unwrap_err().code(),
        "out_of_range"
    );
}

#[test]
fn id_cursors_hold_the_id() {
    let uuid = Uuid::now_v7();
    let cursor = Cursor::from_uuid(uuid);
    assert_eq!(cursor.to_uuid(), Some(uuid));
    assert_eq!(
        Cursor::from_bytes(cursor.as_bytes().to_vec()).unwrap(),
        cursor
    );
    assert!(NewestFirst.accepts(&cursor));

    let other = Cursor::from_bytes(vec![1, 2, 3]).unwrap();
    assert_eq!(other.to_uuid(), None);
    assert!(!NewestFirst.accepts(&other));
}

#[test]
fn cursors_are_bounded() {
    assert_eq!(
        Cursor::from_bytes(Vec::new()).unwrap_err().code(),
        "invalid_cursor"
    );
    assert!(Cursor::from_bytes(vec![0; MAX_CURSOR_BYTES]).is_ok());
    assert!(Cursor::from_bytes(vec![0; MAX_CURSOR_BYTES + 1]).is_err());
}

#[test]
fn keyset_cursors_put_the_sort_key_before_the_id() {
    let updated_at: i128 = 1_750_000_000_000_000_000;
    let id = Uuid::now_v7();
    let cursor = Cursor::keyset(&updated_at.to_be_bytes(), id);

    let (key, back) = cursor.split_keyset().unwrap();
    assert_eq!(back, id);
    assert_eq!(i128::from_be_bytes(key.try_into().unwrap()), updated_at);
    // An id-only cursor is a keyset with an empty key.
    assert_eq!(Cursor::from_uuid(id).split_keyset(), Some((&[][..], id)));
    assert_eq!(Cursor::from_bytes(vec![1]).unwrap().split_keyset(), None);
}

#[test]
fn fetch_limit_is_one_more_than_the_page() {
    let request = PageRequest::<NewestFirst>::new(PageSize::parse(Some(10)).unwrap(), None);
    assert_eq!(request.fetch_limit(), 11);
    assert_eq!(request.after_id(), None);
}

fn cursor_of(n: &u32) -> Cursor {
    #![expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "`Page::from_rows` passes items by reference"
    )]
    Cursor::from_uuid(Uuid::from_u128(u128::from(*n)))
}

#[test]
fn a_full_page_with_an_extra_row_has_a_next_cursor() {
    let request = PageRequest::<NewestFirst>::new(PageSize::parse(Some(3)).unwrap(), None);
    let page = Page::from_rows(vec![9, 8, 7, 6], &request, cursor_of);

    assert_eq!(page.items, [9, 8, 7]);
    assert_eq!(page.next, Some(cursor_of(&7)));
}

#[test]
fn the_last_page_has_no_next_cursor() {
    let request = PageRequest::<NewestFirst>::new(PageSize::parse(Some(3)).unwrap(), None);

    let exact = Page::from_rows(vec![3, 2, 1], &request, cursor_of);
    assert_eq!(exact.items, [3, 2, 1]);
    assert_eq!(exact.next, None);

    let short = Page::from_rows(vec![1], &request, cursor_of);
    assert_eq!(short.next, None);

    let empty = Page::from_rows(Vec::new(), &request, cursor_of);
    assert_eq!(empty, Page::empty());
}

#[test]
fn map_keeps_the_cursor() {
    let request = PageRequest::<NewestFirst>::new(PageSize::parse(Some(1)).unwrap(), None);
    let page = Page::from_rows(vec![2, 1], &request, cursor_of).map(|n| n * 10);
    assert_eq!(page.items, [20]);
    assert_eq!(page.next, Some(cursor_of(&2)));
}
