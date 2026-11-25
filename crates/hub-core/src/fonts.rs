//! Fonts are embedded so boards render identically on desktop and Android.
pub static FONTS: &[(&[u8], &str)] = &[
    (include_bytes!("../assets/fonts/BarlowCondensed-Bold.ttf"), "Barlow Condensed Bold"),
    (include_bytes!("../assets/fonts/BarlowCondensed-SemiBold.ttf"), "Barlow Condensed SemiBold"),
    (include_bytes!("../assets/fonts/BarlowCondensed-Medium.ttf"), "Barlow Condensed Medium"),
    (include_bytes!("../assets/fonts/Inter-Regular.otf"), "Inter Regular"),
    (include_bytes!("../assets/fonts/Inter-Medium.otf"), "Inter Medium"),
    (include_bytes!("../assets/fonts/Inter-SemiBold.otf"), "Inter SemiBold"),
    (include_bytes!("../assets/fonts/Inter-Bold.otf"), "Inter Bold"),
];
