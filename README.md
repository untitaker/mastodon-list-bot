# mastodon-list-bot

> Oh no, the evil algorithm is back!

A small program to generate "programmatic" lists in Mastodon, as a way to
experiment with different kinds of home feeds.

Mastodon's strict chronological timeline favors frequent posters over less
frequent ones. In my experience this causes a problem where your feed is
dominated by a few hardcore, big-follower-number posters, while you're missing
out on content from less frequent posters (such as Twitter holdouts, or your
mutuals).

Mastodon's solution to this is lists, but lists require curation and effort to
maintain. So what if those lists updated themselves?

## Usage

Create an empty list with the name `#last_status_at<1w`. The program will recognize it
by its name, and overwrite its contents with users who haven't posted in a week.

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

Your list is now populated with new accounts. Run this program periodically to
update it (this both adds and removes accounts).

## All supported lists

Currently supported:

* `#last_status_at<1d` -- contains all users which haven't posted in a **day** or more.
* `#last_status_at<1w` -- contains all users which haven't posted in a **week** or more.
* `#last_status_at<1m` -- contains all users which haven't posted in a **month** or more.
* `#mutuals` -- contains all users who you follow and who also follow you.

Variations such as `2d`, `3d`, `8m` are permitted.

List names do not have to match exactly, they only have to end with the
specified string. For example, it is permitted to name a list `My best friends
#mutuals`, so that your preferred list name is shown while the
"machine-readable configuration" is still there. There can currently however
only be one `#` in the name.

List clauses can not be composed, so creating a list of mutuals who haven't
posted in a week is not possible right now.

## Future plans

I want to keep experimenting with this on my own account for now, but am
looking for ways to expose this as some kind of service to other users.

However, this program hammers the API a lot. At a minimum, there would have to
be a way to throttle updates, which currently doesn't exist, and ideally both
on a per-user and a per-instance basis.

## License

MIT
