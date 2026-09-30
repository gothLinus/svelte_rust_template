-- Objects still in the store are not removed by this: empty the bucket by hand.
delete from permissions where name in ('files:read', 'files:write', 'files:manage');

drop trigger files_queue_object_deletion on files;
drop function queue_deleted_file_objects();
drop table object_deletions;
drop table files;
