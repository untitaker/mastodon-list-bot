-- Add migration script here
alter table accounts add column list_count integer not null default -1;
