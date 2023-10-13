# mastodon-list-bot

> Oh no, the evil algorithm is back!

A small program to generate "programmatic" lists in Mastodon, as a way to
experiment with different kinds of home feeds.

Currently supported:

* `#last_status_at<1d` -- contains all users which haven't posted in a day or more.
* `#last_status_at<1w` -- contains all users which haven't posted in a week or more.

## Usage

Create an empty list with one of the above names. The program will recognize it
by its name, and overwrite its contents.

Go to Development in your Mastodon account, and create a new access token.

Export these variables:

```
export MASTODON_INSTANCE=https://mastodon.social
export MASTODON_TOKEN=...
export RUST_LOG=info
```

Then, run:

```
cargo run
```

## License

MIT
