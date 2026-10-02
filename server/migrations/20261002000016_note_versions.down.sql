drop trigger notes_bump_version on notes;
alter table notes drop column version;
drop function bump_version();
