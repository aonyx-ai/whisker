// r[verify lint.derive-order.level]
// r[verify lint.derive-order.std-order]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct CorrectFullStdOrder;

#[derive(Clone, Debug)]
struct CorrectPartialStdOrder;

#[derive(Clone, Debug, Default)]
struct CorrectSubsetStdOrder;

#[derive(Copy, Clone)]
struct CorrectTwoStdDerives;

// r[verify lint.derive-order.detect]
// r[verify lint.derive-order.message]
#[derive(Debug, Clone)] //~ ERROR derive macros are not in canonical order
struct WrongStdOrder;

#[derive(Default, Copy, Clone)] //~ ERROR derive macros are not in canonical order
struct WrongStdOrderMultiple;

#[derive(Hash, Debug, Eq, PartialEq)] //~ ERROR derive macros are not in canonical order
struct WrongStdOrderFour;

// r[verify lint.derive-order.third-party-after-std]
#[derive(Clone, Debug, MyMacro)] //~ ERROR cannot find derive macro `MyMacro`
struct CorrectThirdPartyAfterStd;

#[derive(MyMacro, Clone, Debug)] //~ ERROR derive macros are not in canonical order
//~| ERROR cannot find derive macro `MyMacro`
struct ThirdPartyBeforeStd;

// r[verify lint.derive-order.third-party-alpha]
#[derive(Clone, Debug, Alpha, Beta)] //~ ERROR cannot find derive macro `Alpha`
//~| ERROR cannot find derive macro `Beta`
struct CorrectThirdPartyAlpha;

#[derive(Clone, Debug, Beta, Alpha)] //~ ERROR derive macros are not in canonical order
//~| ERROR cannot find derive macro `Beta`
//~| ERROR cannot find derive macro `Alpha`
struct WrongThirdPartyAlpha;

// r[verify lint.derive-order.detect]
#[derive(Debug, Clone, Beta, Alpha)] //~ ERROR derive macros are not in canonical order
//~| ERROR cannot find derive macro `Beta`
//~| ERROR cannot find derive macro `Alpha`
struct WrongStdAndThirdParty;

#[derive(Debug)]
struct SingleDerive;

fn main() {}
