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
maintain. So what if those lists updated themselves? [Read more about motivation](https://unterwaditzer.net/2023/mastodon-timelines.html)

## How to use

Create these lists to get started:

* `#mutuals` -- contains all users who you follow and who also follow you.
* `#last_status_at>1d & last_status_at<4d` -- contains all users who haven't
  posted yesterday, but sometime within the past three days.
* `#last_status_at>3d` -- contains all users who haven't posted in over three
  days.

Then, in the client of your choice, add those lists as columns or tabs, so you
can easily switch between home timeline and alternative timelines. In Mastodon
web they are already tabs, in [Phanpy](https://phanpy.social/) I recommend the
column layout, in [Tusky](https://github.com/tuskyapp/Tusky/) you can add them
as tabs as well.

Then, head over to [list-bot.woodland.cafe](https://list-bot.woodland.cafe/)
and sign in with your Mastodon account. Click sync, and the bot should start
adding users to the list (asynchronously).

The bot has been successfully tested on GoToSocial as well.

## Syntax reference

* `#last_status_at` supports days (`1d`), weeks (`1w`), months (`1m`). It does
  not support numbers larger than 999 (`9999m` is invalid)
* `#last_status_at` supports operators `<` and `>`. Other operators may be
  added if it's useful, but so far it doesn't seem that it would be.
* `#mutuals` takes no arguments of any kind.
* Clauses can be chained with `&`. Other operators or parenthesis are not
  supported.

List names do not have to match exactly, they only have to end with the
specified string. For example, it is permitted to name a list `My best friends
#mutuals`, so that your preferred list name is shown while the
"machine-readable configuration" is still there. There can currently however
only be one `#` in the name.

## Self-hosting

list-bot comes as a CLI to put into crontab, and as a webservice. For
single-user purposes, it's probably easier to run it from the CLI.

Go to Development in your Mastodon account, and create a new access token.

Then, run:

```
RUST_LOG=info cargo run run-once --host=mastodon.social --token=...
```

Your lists are now populated with new accounts. Run this program periodically
to update it (this both adds and removes accounts).

## Caveats

This bot hammers the API a lot during sync. It is likely that while it is
running, it will encounter rate limits, which it will handle gracefully. Do not
run this program more than once per day.

## License

MIT
