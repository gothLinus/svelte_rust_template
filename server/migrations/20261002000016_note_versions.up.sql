-- Optimistic concurrency. A table with a `version` column gets this trigger next to
-- `set_updated_at`:
--   create trigger <table>_bump_version before update on <table>
--       for each row execute function bump_version();
-- Clients send the version they read (`If-Match`) and a change made against an older one
-- is refused. Like `set_updated_at`, an update that changes nothing keeps the version.
create function bump_version() returns trigger
language plpgsql as $$
begin
    if row(new.*) is distinct from row(old.*) then
        new.version = old.version + 1;
    end if;
    return new;
end;
$$;

alter table notes add column version bigint not null default 1 check (version >= 1);

create trigger notes_bump_version before update on notes
    for each row execute function bump_version();
