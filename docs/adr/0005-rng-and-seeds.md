# 0005. Deterministic RNG with one stream per randomization step

- Status: Accepted

## Context

A seed plus a settings string must reproduce a randomization exactly, on every OS and in every later app version
where possible. With one random generator for the whole run, adding one random call anywhere would change
everything after it. Reproducing the results of other randomizers' seeds is not a goal, because the algorithms
differ in detail anyway.

## Decision

- Use `rand_chacha::ChaCha8Rng`, whose output is specified and stable across platforms and versions.
- The seed is a `u64`. The UI shows and accepts it as a decimal string.
- Each pipeline step gets its own stream: `ChaCha8Rng::seed_from_u64(seed)` with `set_stream(step_id)`. A change
  in one step does not shift the random choices of any other step.
- Randomizers get the RNG only through their step context. The randomization crates do not depend on `rand` or
  `getrandom`, so there is no entropy source to reach for; CI fails if one shows up in their dependency tree.

## Consequences

- A seed made with another randomizer does not reproduce that randomizer's result. Settings strings stay
  compatible.
- Snapshot tests per step stay stable while other steps change.
