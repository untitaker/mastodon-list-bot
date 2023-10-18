create table if not exists accounts
(
    host text not null,
    username text not null,
    token text not null,
    created_at datetime not null,
    last_success_at datetime,
    failure_count integer not null,
    last_error text,
    primary key (host, username)
);
