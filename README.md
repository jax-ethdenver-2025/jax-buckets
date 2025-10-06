# Generic Fullstack

Take a look at the [deployed demo](https://generic.krondor.org)

This repository contains a couple different patterns for building full stack applications I've developed over the years, with focuses on:
- type safe and ergonomic implementation patterns
- rapid iteration and full featured CICD
- owning the deployment pipeline
- and a reliance on running applications in containers for portability

The purpose of it is to track and publish the sum total of experience i have shipping
 quick weekend projects and experiments with the additional satisfaction of doing so
 with my tools of choice. Note, the described methods herin are:
 - **not audited**: use at your own risk
 - **not infinitely scalable**: note the lack of lambdas, external service providers, and orchestrators.
    These templates are meant to provide a base for a proof of concept that you might extend into a long lived
    application. It is up to you to decide on the right deployment surface and implement it. That being
    said, monoliths are pretty much enough for lots of projects, and that's exactly what you get here!
- **unfinished** and likely will always be so. Last year I was writing bespoke ansible and managing my SSH
    keys in my password manager -- things change! be sure to check back for updates, or feel free to contribute one.

For more info on my deployment posture, take a look at [my dev ops docs](./docs/dev-ops/index.md)

## Templates

### Typescript

**Typescript + pnpm + turbo repo**

After many years banging my head against the typescript ecosystem, I finally found a development
 pattern that (more or less) works: `pnpm` for package management and `turbo` for monorepo support.

This template is great for:
- writing simple static or client driven sites
- one off express services or public APIs
- situtations where you need access to typescript
- or otherwise typescript is your team's core skillset

As is, it comes with:
- a simple single page static vite application
- and an express api app with type safe handlers

It could be readily extended to projects such as:
- a crypto app with no backend state
- a quick hackathon project
- or a public portfolio or static blog

I would really like to extend it with:
- some sort of auth pattern, either based on:
   - client-side crytpographic keys
   - a drop in auth proxy
- backend state via either 
    - a sql dialect 
        - (I have yet to find a fully featured and type safe sql tool for typescript projects that I would want to maintain here)
    - or mongo, which is pretty easy to hack with in typescript

### Python

**Python + uv + FastAPI + SqlAchemy + HATEOAS**

I recently fell in love with [HATEOAS](https://en.wikipedia.org/wiki/HATEOAS) as a development pattern 
 for apps built on top of REST APIs. I've had alot of fun using tools like [htmx](https://htmx.org/) and [franken ui](https://franken-ui.dev)
 for building pretty, responsive full stack applications against a server written in the language of your choice.

I decided to start protoyping new ideas entirely in Python using this pattern because:
- there are python libraries for pretty much anything
- it gets the latest and greatest in patterns for working with LLMs
- and declaritive execution is handy when it comes to rapid iteration

At the moment this template includes:
- Handy dev tooling, including hot reloading in response to code changes
- Ready-to-go local postgres for local hacking
- Google OAuth (which is pretty much all you need for a simple product)
- and easy to extend patterns for writing SSR pages and components

This template is great for:
- all sorts of full stack applications
- quick protoyping

I would really like to extend it with:
- Ready-to-go local redis + background jobs using Arq
- common sense LLM patterns + maybe a fly wheel implementation against [Tensor Zero](https://www.tensorzero.com/)

### Rust

TODO: 
if you peak at [my github repos](https://github.com/amiller68?tab=repositories), you can see I have a few opinions on what makes for a manageable full-stack Rust Web App, largely informed by [my friend sam's work](https://github.com/sstelfox/web-app-template). I'll be moving examples of that work here, and it should eventually include am example detailing:
- a full-stack axum + htmx web app
- with OIDC based authorization
- backed by SQLX and [insert-sql-flavor-here]
- and a companion CLI tool template

and maybe some handy patterns for shipping wasm directly to the browser if I get a lil crazy with it.

I don't think this stack is necessarily the best one for quickly protoyping ideas, considering that:
- the Rust ecosystem is still pretty immature, and you won't find the support for the library or framework you need 100% of the time
- type checking, a good generic system, and memory safety are amazing, but sometimes get in the way of moving fast
- good Rust engineers are hard to find, and hard to hire for

BUT its still my favorite in that:
- Rust is incredibly portable
- The community's documentation for crates is unmatched by any other language I've worked with
- Cargo provides an unbeatable developer experience for working with dependencies and feature gaurds


## TODOs

- I think pulumi might actually be a better fit for provisioning the scope of infrastructure we define for a project like this.
# jax-buckets
