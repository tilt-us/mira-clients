# Auto Attacks

Auto attacks are server-authoritative player commands with local client prediction for visuals.

## Range And Speed

- Attack range is `5.0` world units, matching roughly `500` design units.
- Attack speed is resolved from shared champion combo tuning.
- The current prototype tuning uses `1.0` attacks per second for every champion.

## Combo Lengths

Each champion has a repeating auto-attack combo. The first hit deals the least damage and the final
hit deals the most damage.

| Champion | Combo attacks |
| --- | ---: |
| Ignara | 3 |
| Lira | 4 |
| Yuna | 5 |
| Sophia | 3 |

Unknown champions fall back to the minimum combo length of `3`.

Combo progress resets after `2.0` seconds without an accepted auto attack. Every accepted auto
attack refreshes that timer. After the final combo hit, the next accepted attack starts again at the
first combo hit.

## Damage

Combo damage is calculated by the authoritative server using tuning from `mira-game-api`.
Clients only render the replicated combat outcome.

- First combo hit: `6` damage.
- Final combo hit: `18` damage.
- Intermediate hits interpolate linearly between those values.

## Development Dummy

The development dummy is disabled when a development preview opens. Press `F9` to spawn it at the
current player position or remove it during a development preview session. It cannot die locally
and is clamped to at least `1` HP. If it is not hit for `2.0` seconds, it heals back to full health.

The dummy health bar shows accumulated damage as `Total Dmg <amount>`. This label resets and hides
after `10.0` seconds without new damage.

Floating combat text colors:

- Auto attacks: yellow.
- Spell damage: purple.
- Dummy healing: green with a `+` prefix.
