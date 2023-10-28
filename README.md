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

## How to use

Create a list with any of the following names. The bot will then start populating it.

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

## Using the bot as a service

This bot is available as a webservice at
[list-bot.woodland.cafe](https://list-bot.woodland.cafe/). Sign in with
Mastodon, create one of the lists above and click "Sync now" to get started.

## Using the bot from your own machine

Create an empty list with the name `#last_status_at<1w`. The program will recognize it
by its name, and overwrite its contents with users who haven't posted in a week.

Go to Development in your Mastodon account, and create a new access token.

Then, run:

```
RUST_LOG=info cargo run run-once --host=mastodon.social --token=...
```

Your list is now populated with new accounts. Run this program periodically to
update it (this both adds and removes accounts).

## Caveats

This bot hammers the API a lot during sync. It is likely that while it is
running, it will encounter rate limits, which it will handle gracefully. Do not
run this program more than once per day.

## License

MIT
