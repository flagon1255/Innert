Modular RPG Weapon System
Game Design Document

Project: Modular Loot-Based RPG
Design inspiration: Borderlands-style procedural loot, fantasy RPGs, elemental combat
Status: Initial design specification

1. Vision and Goals

Create a modular weapon-generation system in which weapons are assembled from interchangeable parts. Each part can influence weapon stats, appearance, rarity, abilities, and combat behavior.

The goal is to make loot exciting by allowing players to discover unusual combinations instead of simply replacing equipment with items that have higher numbers.

Core principles
Modularity: Weapons consist of independently generated components.
Variety: Components can produce thousands of possible combinations.
Meaningful rarity: Higher rarity introduces interesting mechanics, not just larger numbers.
Synergy: Compatible effects can interact to create powerful combinations.
Build diversity: Different combinations support different playstyles.
Visual identity: Parts influence the appearance of the assembled weapon.
Balance: Every component has a purpose, and no single rarity is automatically best for every build.
2. Weapon Categories
2.1 Melee Weapons

Supported weapon families:

Swords
Greatswords
Daggers
Axes
Great axes
Hammers
Spears
Polearms
Claws
Scythes
Whips
Fist weapons

Potential component slots:

Slot	Primary purpose
Weapon frame	Defines weapon family and base behavior
Blade or head	Damage, reach, attack pattern
Handle or hilt	Attack speed, combos, handling
Guard or pommel	Critical chance, defense, parrying
Core	Elemental damage and special effects
Rune	Passive abilities and combat modifiers

Not every melee weapon must use every slot. Daggers, hammers, and spears can have different component layouts.

2.2 Ranged Weapons

Supported weapon families:

Bows
Crossbows
Throwing weapons
Firearms, if supported by the setting
Magical projectile launchers

Potential component slots:

Slot	Primary purpose
Frame	Base damage and weapon behavior
Barrel or limbs	Range, projectile speed, accuracy
Grip or stock	Stability, handling
String or magazine	Draw speed, capacity, reload behavior
Firing mechanism	Burst fire, piercing, multishot
Core	Elemental or magical properties
Scope or sight	Accuracy and critical bonuses
2.3 Magic Weapons

Supported weapon families:

Staves
Wands
Grimoires
Spell blades
Magical orbs
Catalysts
Magical foci

Potential component slots:

Slot	Primary purpose
Base frame	Casting style and weapon family
Focus	Spell family or school
Crystal	Element and damage type
Conduit	Casting speed and mana cost
Catalyst	Spell modifiers
Binding	Passive effects and summons
Rune	Unique magical behavior

Magic weapons may generate projectiles, area attacks, beams, summons, buffs, or other spell effects.

3. Component Data Model

Every component should be defined as data rather than implemented as a separate hardcoded weapon.

Each component should support the following fields:

Field	Description
id	Unique component identifier
displayName	Name shown to the player
slotType	Component slot
compatibleFamilies	Weapon families that can use it
rarity	Component rarity
baseStats	Flat stat modifications
statMultipliers	Percentage-based modifications
effects	Granted abilities or passive effects
synergyTags	Keywords used for effect interactions
visualProfile	Appearance, materials, and particles
weight	Relative probability in loot selection
levelRequirement	Minimum required level
uniqueRules	Special compatibility or generation rules
Example component
json
{
  "id": "infernal_blade",
  "displayName": "Infernal Blade",
  "slotType": "blade",
  "rarity": "legendary",
  "compatibleFamilies": [
    "sword",
    "greatsword"
  ],
  "baseStats": {
    "fireDamage": 24
  },
  "statMultipliers": {
    "physicalDamage": 1.15
  },
  "effects": [
    "apply_burn"
  ],
  "synergyTags": [
    "fire",
    "burn",
    "heavy_attack"
  ],
  "weight": 5,
  "levelRequirement": 10
}


This is illustrative JSON. The final implementation should use the conventions of the selected game engine.

4. Rarity System

Initial rarity tiers:

Rarity	Suggested color	Design purpose
Common	Gray	Basic stats and reliable performance
Uncommon	Green	Minor bonuses or simple passives
Rare	Blue	Distinctive modifiers
Epic	Purple	Strong effects and meaningful interactions
Legendary	Orange	Signature mechanics that change playstyle
Mythic	Red or pink	Exceptional mechanics and rare transformations
Rarity rules
Each component has its own rarity.
Weapon rarity is calculated separately from individual component rarity.
A single Legendary component can give an ordinary weapon a distinctive identity.
Component rarity affects drop probability, available power budget, and possible effect complexity.
Higher rarity does not automatically mean higher performance in every build.
Components can have level restrictions independent of rarity.
Example weapon composition
Common blade
Rare hilt
Epic core
Legendary rune

This weapon could have ordinary physical damage, improved attack speed, void damage, and a powerful signature ability.

Its overall quality should reflect its component composition and actual capabilities, rather than merely averaging rarity tiers.

Initial rarity probabilities

These values are starting assumptions for testing, not final balance targets.

Rarity	Base drop probability
Common	55%
Uncommon	25%
Rare	13%
Epic	5%
Legendary	1.8%
Mythic	0.2%
Total	100%

These probabilities can be modified by enemy type, difficulty, player luck, location, and special events.

Rarity rolls should generally happen per component, subject to compatibility and loot rules.

5. Stat System

All weapons should use a shared stat framework where possible.

Offensive stats
Physical damage
Elemental damage
Attack speed
Critical chance
Critical damage
Attack range
Projectile speed
Projectile count
Armor penetration
Status buildup
Stagger strength
Magical stats
Spell damage
Casting speed
Mana cost
Cooldown reduction
Elemental potency
Area of effect
Spell duration
Summon strength
Status-effect duration
Utility stats
Lifesteal
Stamina cost
Reload speed
Draw speed
Movement speed while attacking
Resource generation
Knockback resistance
Effect activation chance
Stat calculation

A basic damage calculation could follow this structure:

text
BaseDamage = FrameDamage + ComponentFlatDamage

ModifiedDamage =
    BaseDamage
    * ComponentMultipliers
    * CharacterMultipliers

FinalDamage =
    ApplyResistance(ModifiedDamage, Target)


Use a consistent calculation order across all weapon families.

For multiple percentage modifiers, define whether they stack additively or multiplicatively. Multiplicative stacking can create extreme damage growth, so it should be monitored carefully.

6. Special Effect Library

Effects should be implemented as reusable systems that components can reference.

6.1 Elemental Effects

Fire

Applies burn damage over time.
Can ignite nearby enemies.
Can create explosions through special synergies.

Frost

Slows enemies.
Builds freeze accumulation.
Can freeze enemies when a threshold is reached.

Lightning

Chains damage between enemies.
Can interrupt casting.
Can charge enemies for subsequent attacks.

Poison

Applies damage over time.
Can stack poison buildup.
Can spread through certain effects.

Arcane

Improves magical damage.
Can generate spell echoes.
Can interact with mana and cooldown systems.

Void

Deals specialized magical damage.
Can trigger implosions or damage bursts.
Can interact with curses and other status effects.
6.2 Physical Effects
Bleeding: damage over time.
Armor break: reduces enemy defense.
Stagger: interrupts or destabilizes enemies.
Knockback: pushes enemies away.
Execute: deals bonus damage against weakened targets.
Wound: reduces healing received.
Rend: increases subsequent physical damage.
6.3 Combat Effects
Lifesteal
Critical hit chains
Combo stacking
Bonus damage against marked targets
Resource regeneration on kills
Temporary attack-speed increases
Damage scaling with missing health
Increased damage against bosses
6.4 Projectile Effects
Multishot
Piercing
Ricochet
Homing
Explosive projectiles
Splitting projectiles
Returning projectiles
Delayed detonation
Projectile duplication
6.5 Magical Effects
Spell echo
Spell duplication
Mana drain
Reduced mana cost
Summoned weapons
Temporary familiars
Spell-triggered explosions
Element conversion
Area expansion
6.6 Risk and Reward Effects
Increased damage at low health.
Higher critical chance at the cost of defense.
Damage amplification that consumes mana.
Increased attack speed with reduced accuracy.
Explosions that also damage the wielder.
Powerful effects with cooldowns or resource costs.

Effects should have standardized parameters, activation rules, durations, cooldowns, and stacking behavior.

7. Synergy System

Synergies allow components to interact based on shared tags and defined conditions.

Example synergy: Solar Collapse

Required tags:

fire
burn
void

Behavior:

The weapon applies burn to an enemy.
A compatible component adds void damage.
When the enemy is staggered, the weapon consumes the burn effect.
A void explosion occurs.
Enemies killed by the explosion may trigger additional effects, subject to a chain limit.
Example synergy: Storm Conductor

Required tags:

lightning
projectile
chain

Behavior:

The weapon fires a lightning projectile.
The projectile chains between nearby enemies.
Each chained hit increases the next hit's damage up to a maximum.
Repeated chain activations are limited to prevent infinite loops.
Example synergy: Blood Reaper

Required tags:

bleed
critical
lifesteal

Behavior:

Attacks apply bleeding.
Critical hits against bleeding enemies restore health.
Killing a bleeding enemy grants a temporary attack-speed bonus.
Synergy implementation rules
Synergies should be data-driven.
Each synergy must define its requirements and activation condition.
Multiple synergies may coexist on a weapon.
Synergies must have clear visual and audio feedback.
Chain reactions must have limits to prevent infinite activation.
Effects should not recursively trigger themselves unless explicitly designed to do so.
Synergy power should be considered when calculating overall weapon strength.
8. Procedural Weapon Generation

Weapon generation should use a controlled pipeline.

Generation pipeline
Determine the player's level and loot source.
Select a weapon family.
Select a compatible base frame.
Determine the available component slots.
Roll a rarity for each slot.
Select compatible components using weighted probabilities.
Apply component stats and effects.
Find valid synergies.
Calculate the weapon's final stats and overall grade.
Validate the weapon against balance constraints.
Generate a name.
Generate visual properties.
Save the resulting weapon instance.
Pseudocode
text
function GenerateWeapon(level, lootContext):
    family = SelectWeaponFamily(lootContext)

    frame = SelectCompatibleFrame(
        family,
        level,
        lootContext
    )

    weapon = CreateWeaponInstance(frame)

    for slot in frame.componentSlots:
        rarity = RollRarity(lootContext)

        component = SelectCompatibleComponent(
            family,
            slot,
            rarity,
            level
        )

        weapon.AddComponent(component)

    weapon.ApplyComponentStats()
    weapon.CollectEffects()
    weapon.ResolveSynergies()
    weapon.CalculateOverallGrade()
    weapon.ValidateBalance()
    weapon.GenerateName()
    weapon.GenerateAppearance()

    return weapon


If a valid component cannot be found, the generator should fall back to a compatible lower-rarity component or another predefined safe option.

Generation should be deterministic when supplied with the same seed and configuration. This makes bugs easier to reproduce.

9. Loot Probability and Quality Control

A component's probability should depend on more than rarity alone.

Possible factors:

Base rarity weight
Player level
Enemy or chest type
Difficulty
Region or biome
Item category
Luck modifiers
Quest rewards
Special loot tables

Example weighted selection:

text
AdjustedWeight =
    BaseWeight
    * ContextMultiplier
    * LuckMultiplier


Normalize the resulting weights before selecting a component.

Luck should not necessarily affect every loot property. It can influence rarity while leaving component type selection unchanged.

Pity systems

Optional mechanics:

Guarantee a Rare or better drop after a defined number of low-rarity drops.
Increase Legendary odds during special encounters.
Guarantee a unique component from a boss.
Prevent excessive duplicate drops in certain reward contexts.

Pity rules should be explicit and testable.

10. Weapon Naming System

Weapon names should be generated from component properties, synergies, and weapon families.

Naming structure
text
[Prefix] + [Core Name] + [Suffix]


Examples:

Ashen Stormcleaver
Widowmaker of the Deep
Frostbite, the Silent Fang
Eclipse, World-Eater
Thunder King's Recurve
Void-Touched Spellbinder

Naming rules:

Legendary and Mythic components may provide signature names.
Synergies can override the normal name.
Names should remain readable and recognizable.
Duplicate names are acceptable unless uniqueness is a deliberate feature.
Generated names should not be used as unique identifiers.
11. Visual Customization

Weapon components should influence appearance independently where possible.

Visual properties
Mesh or sprite
Material
Texture
Glow color
Particle effects
Trail effects
Projectile appearance
Sound effects
Attack animation modifiers
Rarity-specific visual details
Visual generation rules
Each component provides a visual profile.
Compatible profiles can be combined into an assembled weapon.
Higher rarity can add visual effects without requiring a completely separate model.
Legendary components should have recognizable visual signatures.
Visual combinations must be validated for clipping and attachment compatibility.
Gameplay readability takes priority over excessive visual effects.
12. Weapon Instance and Inventory Data

Component definitions should be separated from generated weapon instances.

A weapon instance stores the selected components and any persistent randomized values.

Example:

json
{
  "instanceId": "generated_weapon_001",
  "weaponFamily": "greatsword",
  "itemLevel": 28,
  "componentIds": [
    "ashcleaver_blade",
    "executioner_hilt",
    "void_heart",
    "eclipse_rune"
  ],
  "seed": 184729,
  "overallGrade": "legendary"
}


The instance identifier must be unique within the relevant save or inventory context.

Do not duplicate full component definitions inside every weapon instance. Store references to component definitions, plus rolled values that must persist.

Persist any randomized affixes, component variants, durability values, or other state that cannot safely be reconstructed from the seed.

13. Example Generated Weapons
13.1 Eclipse, World-Eater

Family: Greatsword
Overall grade: Legendary
Combat style: Slow, heavy attacks

Components:

Ashcleaver Blade
Executioner's Hilt
Void Heart
Eclipse Rune

Effects:

High physical damage.
Applies burn.
Deals bonus damage to staggered enemies.
Kills may trigger void bursts.
Burning enemies take additional void damage.

Signature synergy: staggering a burning enemy triggers a void explosion.

13.2 Stormneedle

Family: Bow
Overall grade: Epic
Combat style: Fast precision attacks

Components:

Recurve Frame
Stormstring
Conductive Limbs
Chainshot Mechanism

Effects:

Increased draw speed.
Lightning damage on charged shots.
Projectiles can chain between nearby targets.
Critical hits build electrical charge.
13.3 Gravewhisper

Family: Staff
Overall grade: Legendary
Combat style: Summoning and damage over time

Components:

Bonewood Frame
Soul Crystal
Grave Conduit
Necromancer's Binding

Effects:

Spells apply a soul mark.
Marked enemies generate additional resources when defeated.
Kills have a chance to summon a temporary spectral ally.
Summoned allies inherit a portion of the staff's magical bonuses.
14. Balance Guidelines

The system must prevent powerful combinations from invalidating all other weapons.

Recommended safeguards
Use internal power budgets for components.
Limit stacking of repeated effects where appropriate.
Add cooldowns to high-impact effects.
Cap critical chance, attack speed, or other runaway stats where necessary.
Define resistance and immunity rules.
Prevent unlimited projectile or explosion chains.
Account for effects that increase damage indirectly.
Test single-target and area damage separately.
Test boss encounters separately from groups of weak enemies.
Measure actual combat performance rather than relying exclusively on displayed item scores.

A powerful weapon should be exciting, but it should not eliminate meaningful build choices.

15. Testing Requirements
Generation tests
Every generated weapon has a valid frame.
Every component is compatible with its assigned slot.
Required components are present.
Invalid combinations fall back safely.
Rarity probabilities match configured weights over large samples.
Identical seeds reproduce identical results.
Combat tests
Effects activate under the correct conditions.
Status effects use consistent stacking rules.
Synergies cannot trigger infinite loops.
Damage calculations are consistent across weapon families.
Enemy resistance and immunity are respected.
Multiplayer or replicated combat, if supported, produces consistent results.
Balance tests
Compare average damage across rarity tiers.
Compare melee, ranged, and magic performance.
Evaluate single-target damage and area damage.
Test effects across early, middle, and late game.
Track the frequency of dominant combinations.
Ensure common components remain useful in appropriate builds.
16. Suggested Development Roadmap
Phase 1: Core Framework
Define weapon families.
Implement component definitions.
Implement slot compatibility.
Implement weapon instances.
Implement basic stat calculation.
Phase 2: Loot Generation
Add weighted rarity rolls.
Add compatible component selection.
Add seeded generation.
Add loot source configuration.
Add generated item names.
Phase 3: Combat Effects
Implement elemental damage.
Implement status effects.
Implement critical hits and lifesteal.
Implement projectile modifiers.
Implement effect activation and cooldown rules.
Phase 4: Synergy System
Add synergy tags.
Add condition-based interactions.
Add recursion and chain limits.
Add visual and audio feedback.
Phase 5: Presentation
Combine component meshes or sprites.
Add materials and rarity visuals.
Add projectile and impact effects.
Display component details in the inventory UI.
Phase 6: Balance and Expansion
Build automated generation tests.
Analyze loot probabilities.
Adjust component power budgets.
Add more components and weapon families.
Add unique boss parts and special loot tables.
17. Initial Scope Recommendation

Start with a manageable prototype.

Suggested initial content:

3 weapon families: sword, bow, staff.
4 component slots per family.
10 components per slot.
6 rarity tiers.
15 reusable combat effects.
5 defined synergies.

This provides meaningful variation without requiring a massive content library before the system becomes playable.

Expand the component library after the generation, compatibility, and combat systems are reliable.

18. Decisions Still Required

The following decisions should be finalized before production:

Real-time, turn-based, or hybrid combat.
Single-player or multiplayer support.
Whether components can be removed and swapped - no, for now.
Whether components can be upgraded or crafted - yes
Whether weapons have durability.
Whether components are tradable.
Whether duplicate components can be combined. - no
Whether legendary components have fixed signature effects.
Whether Mythic items are procedurally generated or handcrafted.
Whether the game includes firearms or focuses exclusively on fantasy weapons.
Final Design Principle

The goal is not simply to generate more weapons. The goal is to generate more meaningful decisions.

Every component should contribute to a weapon's identity, and the most memorable discoveries should come from interactions that players can understand, experiment with, and build around.