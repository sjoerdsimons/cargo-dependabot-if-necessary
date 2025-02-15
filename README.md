# cargo-dependabot-if-necessary

For most of my crates i would love to use dependabots "increase-if-necessary"
strategy, unfortunately that isn't supported.

See [dependabot-core!4009](https://github.com/dependabot/dependabot-core/issues/4009) for more details

This little tool updates or generate a dependabot file with the cargo
ecosystems version ignores in a way to mimic the increase-if-necessary strategy.

See this crates own [dependabot.yml](.github/dependabot.yml) for an example of the output.
