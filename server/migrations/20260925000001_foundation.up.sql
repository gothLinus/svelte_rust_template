-- Shared building blocks for the other migrations.

-- Keeps `updated_at` current on every table that has one. Attach it with
--   create trigger <table>_set_updated_at before update on <table>
--       for each row execute function set_updated_at();
-- An update that changes nothing leaves the timestamp alone.
create function set_updated_at() returns trigger
language plpgsql as $$
begin
    if row(new.*) is distinct from row(old.*) then
        new.updated_at = now();
    end if;
    return new;
end;
$$;
