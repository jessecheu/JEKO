//! The shop at a safe stop (FR17, FR28): where the gears picked up along the
//! road are spent. Pure rules, no drawing and no input. `Game::buy` is the one
//! place a purchase happens, and it mends hearts through `Health`
//! (invariant 6), so this module only says what a thing costs and whether it
//! can be bought.

use super::health::Health;

/// What the shop sells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// Hot soup: two hearts back at once.
    Soup,
    /// A heart container: one more heart, for the rest of the run.
    Heart,
}

impl Item {
    pub const ALL: [Item; 2] = [Item::Soup, Item::Heart];

    /// Gears it costs.
    pub fn cost(self) -> u32 {
        match self {
            Item::Soup => 4,
            Item::Heart => 12,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::Soup => "Hot soup",
            Item::Heart => "Heart charm",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Item::Soup => "Mends two hearts at once.",
            Item::Heart => "One more heart, for the rest of this run.",
        }
    }
}

/// Why an item can't be bought right now, if it can't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not enough gears.
    TooPoor,
    /// Nothing for it to do: hearts are full.
    NotHurt,
    /// Already at the most hearts.
    AtMax,
}

impl Refusal {
    pub fn reason(self) -> &'static str {
        match self {
            Refusal::TooPoor => "Not enough gears",
            Refusal::NotHurt => "Your hearts are full",
            Refusal::AtMax => "You can't carry more hearts",
        }
    }
}

/// Whether `item` can be bought with `gears` by a player in this `health`.
pub fn check(item: Item, gears: u32, health: &Health) -> Result<(), Refusal> {
    match item {
        Item::Soup if health.current >= health.max => return Err(Refusal::NotHurt),
        Item::Heart if health.hearts() >= super::health::MAX_HEARTS => return Err(Refusal::AtMax),
        _ => {}
    }
    if gears < item.cost() {
        return Err(Refusal::TooPoor);
    }
    Ok(())
}

/// Buys `item`: takes the gears and applies it. Returns the refusal, with
/// nothing changed, if it can't be bought.
pub fn buy(item: Item, gears: &mut u32, health: &mut Health) -> Result<(), Refusal> {
    check(item, *gears, health)?;
    *gears -= item.cost();
    match item {
        Item::Soup => health.heal(4),
        Item::Heart => {
            health.add_heart();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hurt() -> Health {
        let mut h = Health::new(5);
        h.damage(5);
        h
    }

    #[test]
    fn soup_mends_two_hearts_and_costs_gears() {
        let (mut gears, mut h) = (10, hurt());
        assert_eq!(buy(Item::Soup, &mut gears, &mut h), Ok(()));
        assert_eq!(gears, 6);
        assert_eq!(h.current, 5 + 4);
    }

    #[test]
    fn nothing_changes_when_it_is_refused() {
        let (mut gears, mut h) = (3, hurt());
        assert_eq!(buy(Item::Soup, &mut gears, &mut h), Err(Refusal::TooPoor));
        assert_eq!((gears, h.current), (3, 5));
        let (mut gears, mut full) = (50, Health::new(5));
        assert_eq!(
            buy(Item::Soup, &mut gears, &mut full),
            Err(Refusal::NotHurt)
        );
        assert_eq!(gears, 50);
    }

    #[test]
    fn a_heart_charm_grows_the_maximum_up_to_the_cap() {
        let (mut gears, mut h) = (1000, Health::new(5));
        for _ in 0..3 {
            assert_eq!(buy(Item::Heart, &mut gears, &mut h), Ok(()));
        }
        assert_eq!((h.hearts(), h.current, h.max), (8, 16, 16));
        assert_eq!(buy(Item::Heart, &mut gears, &mut h), Err(Refusal::AtMax));
        assert_eq!(gears, 1000 - 3 * Item::Heart.cost());
    }
}
