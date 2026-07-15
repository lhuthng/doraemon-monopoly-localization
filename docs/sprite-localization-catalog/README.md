# Sprite1.dat localization catalogue

This catalogue lists visually confirmed `Sprite1.dat` records containing baked
language. IDs are archive record IDs, matching the Graphics studio and exported
PNG filenames. Geometry alone is not treated as proof that a sprite is text.

## Localization targets

| Priority | Sprite IDs    | Graphics studio page | What is visible                                                                                                                   | Recommendation                                                                                                                   |
| -------- | ------------- | -------------------: | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| High     | `10535–10543` |                  106 | Animated mode submenu: `比賽模式` (Competition Mode) and `練習模式` (Practice Mode)                                               | Localize both entries across all nine frames.                                                                                    |
| High     | `10544–10552` |                  106 | Animated two-item submenu: `新遊戲` (New Game) and `載入進度` (Load Progress)                                                     | Localize both entries across all nine frames.                                                                                    |
| High     | `10553–10561` |                  106 | Animated title-screen main menu: `大富翁`, `小遊戲`, `操控設定`, `排行榜`, and `離開遊戲`                                         | This is the five-item menu shown on the title screen. `10553` is the blank/start frame; `10560–10561` contain the complete menu. |
| High     | `11087–11113` |              111–112 | Animated map-selection artwork containing a Chinese map label followed by map numbers                                             | Localize the label consistently across all 27 frames.                                                                            |
| High     | `11118–11146` |                  112 | Scrolling `回主選單` (Return to Main Menu) sign                                                                                   | Localize the same phrase across all 29 animation frames.                                                                         |
| High     | `11687`       |                  116 | Large `地圖選擇` (Map Select) heading                                                                                             | Replace with the localized map-selection heading.                                                                                |
| High     | `13682–13699` |                  136 | Two directions of an animated three-item submenu: `新遊戲` (New Game), `載入進度` (Load Progress), and `成績單` (Records/Results) | This is a submenu, not the five-item title menu. Localize all three entries across both nine-frame directions.                   |
| Optional | `13772–13796` |              136–137 | `STAFF LIST` followed by individual staff names                                                                                   | These already use Latin letters. Edit only if the credits need localization or corrected romanization.                           |
| High     | `13798–13806` |                  137 | Animated two-item submenu: `比賽模式` (Competition Mode) and `排行榜` (Ranking)                                                   | Localize both entries across all nine frames.                                                                                    |

For the page number above, the viewer uses 96 decoded sprites per page. Searching
for the exact ID remains the safest way to reach a record because unsupported
archive records are omitted from the decoded grid.

## Reviewed false positives

These ranges scored like text because they are wide, sparse, low-colour images,
but visual review shows that they do not contain language:

| Sprite IDs    | Actual content                                                      |
| ------------- | ------------------------------------------------------------------- |
| `2582–2599`   | Speech-bubble/window animation with decorative icons; no baked text |
| `5795–5823`   | Horizontal meter/frame animation                                    |
| `10603–10660` | Character and effect animation                                      |
| `10904–10992` | Repeated map/island and oval-frame animation                        |
| `11741–11769` | Horizontal meter animation                                          |
| `13499–13599` | Recoloured horizontal meter families                                |
| `13700–13731` | Checkerboard/window construction frames                             |

Other isolated high-scoring records such as `792`, `1699`, `2342`, `2374`,
`5941`, `6747`, `9134–9135`, `9775`, `9782`, `10280–10281`, `13315`, and
`13746` are artwork, empty frames, roads, panels, or meters rather than text.

## Supporting file

`sprite-index.csv` contains all 13,069 decoded sprites with dimensions,
hotspots, visible-pixel counts, visible palette-slot counts, aspect ratio, and
the visually confirmed localization-family classification where applicable.

## Scope warning

This list covers language baked into `Sprite1.dat`. Main-menu labels, map names,
and other large fixed UI text can instead be baked into `bitmaps.dat`; dialogue
and game messages primarily come from `strings.dat` and the font archives.
