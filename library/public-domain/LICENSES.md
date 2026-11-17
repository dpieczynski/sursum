# Library sources and licenses

Every file under `library/public-domain/Songs/` must be listed here with its source and status.
Allowed statuses:

- **public-domain**: text (and, where relevant, melody) whose author died more than 70 years ago,
  taken from a public-domain source (e.g. Wikisource), with the source URL;
- **synthetic**: text written for this project to exercise a parser rule; no real song.

Copyrighted texts are never allowed, including excerpts. Real parish libraries are tested locally
from `/local/` (git-ignored), see [docs/testing.md](../../docs/testing.md).

## Rules applied to this library (checked on 2026-11-17)

Under Polish law economic rights last 70 years from the end of the year of the author's death; for
anonymous works, 70 years from first publication. A text is included only if

- every author and every translator of the text died in 1955 or earlier, with the death year
  confirmed from a reliable source (Wikisource author page, Polski Słownik Biograficzny, library
  authority record); or
- it is anonymous or traditional and the text is taken from a printed edition published in 1955
  or earlier.

The wording always comes from an old public-domain edition (here: 1838–1850), never from a modern
songbook or website; modern sources were used only to find titles. Only spelling is modernized
(e.g. `czem` → `czym`, `-emi` → `-ymi`, `Xiężna` → `Księżna`, `zkąd` → `skąd`, separate `nie` with
verbs); every change is listed per file below and in the file's hidden `;Zmiany:` comment.
Transcriptions on Wikisource were checked against the scans where their pages are not proofread,
and transcription errors found were corrected from the scan (listed per file). Texts that were
considered and left out are listed in [REJECTED.md](REJECTED.md).

## Main source

- **M. M. Mioduszewski (1787–1868), *Śpiewnik kościelny czyli pieśni nabożne z melodyjami w
  kościele katolickim używane*, Kraków 1838** (druk S. Gieszkowski), bound with *Dodatek do
  Śpiewnika kościelnego* (imprimatur Warszawa 1841, printed pages 387–756) and *Dodatek II do
  Śpiewnika kościelnego* (imprimatur Kraków 1849). Scan: Kujawsko-Pomorska Biblioteka Cyfrowa
  <http://kpbc.umk.pl/dlibra/doccontent?id=51916>, on Commons
  <https://commons.wikimedia.org/wiki/File:Śpiewnik_kościelny_czyli_pieśni_nabożne_z_melodyjami_w_kościele_katolickim_używane_(Mioduszewski).djvu>;
  transcription: <https://pl.wikisource.org/wiki/Śpiewnik_kościelny>. Page numbers below are
  printed page numbers, followed by the scan page.

## Files

| Path | Status | Author | Source edition | Changes |
|---|---|---|---|---|
| `Songs/Adwent/Archanioł Boży Gabriel` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni adwentowe, pieśń VI, s. 17–20 (skan 23–26), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Archanioł_Boży_Gabryel> | pisownia uwspółcześniona (Gabryel → Gabriel, kniej → k niej, kniemu → k niemu, Gabryelu → Gabrielu); zachowano dawne formy: Maryjej, Nalazlaś, Ze krwie |
| `Songs/Adwent/Boże wieczny, Boże żywy` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni adwentowe, pieśń II „Hejnał roratny”, s. 11–12 (skan 17–18), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Boże_wieczny,_Boże_żywy> | pisownia uwspółcześniona (Nietrzymajcie → Nie trzymajcie, wdzięcznemi → wdzięcznymi, świętemi → świętymi, anielskiemi → anielskimi) |
| `Songs/Adwent/Głos wdzięczny z nieba wychodzi` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni adwentowe, pieśń V, s. 15–17 (skan 21–23), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Głos_wdzięczny_z_Nieba_wychodzi> | pisownia uwspółcześniona (knam → k nam, czem → czym); zachowano dawne formy: Myślił, służka, Naszę (biernik) |
| `Songs/Adwent/Hejnał wszyscy zaśpiewajmy` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni adwentowe, pieśń I „Hejnał roratny”, s. 9–10 (skan 15–16), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Hejnał_wszyscy_zaśpiewajmy> | pisownia uwspółcześniona (kniemu → k niemu); zachowano dawne formy: zniosł, wsze |
| `Songs/Do świętych/Cześć winną tobie, Stróżu mój Aniele` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni o Świętych Pańskich, pieśń IV „O Świętym Aniele Stróżu”, s. 221 (skan 227), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Cześć_winną_tobie_Stróżu_mój_Aniele> | pisownia uwspółcześniona (wszystkiem → wszystkim) |
| `Songs/Eucharystyczne/Bądź pozdrowione Serce mego Pana` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni na Boże Ciało, pieśń XVII „O Sercu Pana Jezusa”, s. 174–175 (skan 180–181), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Bądź_pozdrowione_Serce_mego_Pana> | pisownia uwspółcześniona (Wskróś → Wskroś) |
| `Songs/Eucharystyczne/Chwalmy niewysławiony` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni na Boże Ciało, pieśń IX, s. 157–158 (skan 163–164), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Chwalmy_niewysławiony_Sakrament_wystawiony> | pisownia uwspółcześniona (Dobrem wielkiem → Dobrym wielkim) |
| `Songs/Kolędy/A czemuż mój Jezus tak ubogo leży` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni na Boże Narodzenie, pieśń XVII, s. 56–57 (skan 62–63), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/A_czemuż_mój_Jezus_tak_ubogo_leży> | pisownia uwspółcześniona (dla tego → dlatego) |
| `Songs/Kolędy/Ach, ubogi żłobie` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Dodatek II (imprimatur 1849), Pieśni na Boże Narodzenie, pieśń III, s. 773–774 (skan 779–780), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Ach_ubogi_żłobie> | pisownia jak w druku |
| `Songs/Kolędy/Anioł pasterzom mówił` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni na Boże Narodzenie, pieśń II, skan s. 34–35, <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Anioł_pasterzom_mówił> | pisownia uwspółcześniona (podłem → podłym, Niemiał → Nie miał, Któraby nieustała → Która by nie ustała) |
| `Songs/Kolędy/Bóg się rodzi` | public-domain | Franciszek Karpiński (1741–1825) | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni na Boże Narodzenie, pieśń XI, s. 45–47 (skan 51–53), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Bóg_się_rodzi> | pisownia uwspółcześniona (czem → czym, wieśniaczemi → wieśniaczymi, iej → jej); zachowano dawne formy: Dziecie |
| `Songs/Kolędy/Bóg się z Panny narodził` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Dodatek II (imprimatur 1849), Pieśni na Boże Narodzenie, pieśń IX, s. 781–782 (skan 787–788), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Bóg_się_z_Panny_narodził> | pisownia jak w druku; refren oznaczony w druku „Tego dnia wesołego i t. d.” zapisany raz jako [C] |
| `Songs/Kolędy/Figurowana różdżka zielona` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Dodatek II (imprimatur 1849), Pieśni na Boże Narodzenie, pieśń XVII, s. 792–793 (skan 798–799), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Figurowana_różdżka_zielona> | pisownia jak w druku |
| `Songs/Kolędy/Gdy się Chrystus rodzi` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Dodatek II (imprimatur 1849), Pieśni na Boże Narodzenie, pieśń II, s. 772–773 (skan 778–779), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Gdy_się_Chrystus_rodzi> | pisownia uwspółcześniona (czemprędzej → czym prędzej, godnemi → godnymi) |
| `Songs/Maryjne/Anioł Gabriel w poselstwie przychodzi` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni o Najświętszej Maryi Pannie, pieśń IV „Na Zwiastowanie N. Maryi Panny”, s. 181–182 (skan 187–188), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Anioł_Gabryel_w_poselstwie_przychodzi> | pisownia uwspółcześniona (Gabryel → Gabriel, Messyjasza → Mesjasza) |
| `Songs/Maryjne/Ciebie na wieki wychwalać będziemy` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni o Najświętszej Maryi Pannie, pieśń XIII, s. 192–193 (skan 198–199), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Ciebie_na_wieki_wychwalać_będziemy> | pisownia uwspółcześniona (fórto → furto); poprawiono błąd transkrypcji Wikiźródeł (lilika → lilija); zachowano dawne formy: Monarchinią |
| `Songs/Maryjne/Czystsza nad słońce oczyszczenia czeka` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni o Najświętszej Maryi Pannie, pieśń VI „Na Oczyszczenie N. Maryi Panny albo Gromnice”, s. 184 (skan 190), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Czystsza_nad_słońce_oczyszczenia_czeka> | pisownia uwspółcześniona (grzesznemi → grzesznymi, innemi → innymi) |
| `Songs/Ogólne/Boże w dobroci nigdy nieprzebrany` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni przygodne, pieśń III „Zawierająca Akt Skruchy”, s. 250–251 (skan 256–257), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Boże_w_dobroci_nigdy_nieprzebrany> | pisownia uwspółcześniona (dla tego → dlatego, Dla tego → Dlatego); zachowano dawne formy: wolą (biernik) |
| `Songs/Ogólne/Boże z twoich rąk żyjemy` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni przygodne, pieśń XVII „Pod czas pracy w polu”, s. 272–273 (skan 278–279), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Boże_z_twoich_rąk_żyjemy> | pisownia uwspółcześniona (naszemi → naszymi, twem → twym) |
| `Songs/Ogólne/Będę cię wielbił, mój Panie` | public-domain | Jan Kochanowski (1530–1584) | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni przygodne, pieśń XI „O Miłosierdziu Boskiem. Ps. 29”, s. 265–266 (skan 271–272), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Będę_cię_wielbił_mój_Panie> | pisownia jak w druku; poprawiono błąd transkrypcji Wikiźródeł (Boźe → Boże) |
| `Songs/Wielkanoc/Chrystus Pan zmartwychwstał` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni wielkanocne, pieśń V, s. 133–134 (skan 139–140), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Chrystus_Pan_zmartwychwstał> | pisownia uwspółcześniona (Zwycięztwo → Zwycięstwo) |
| `Songs/Wielkanoc/Chrystus zmartwychwstan jest` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni wielkanocne, pieśń II, s. 129–130 (skan 135–136), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Chrystus_zmartwychwstan_jest> | pisownia uwspółcześniona (zmartwych → z martwych); zachowano dawne formy: nodze, Wstałci |
| `Songs/Wielki Post/Ach mój Jezu, jak ty klęczysz` | public-domain | tekst tradycyjny | M. M. Mioduszewski, Śpiewnik kościelny, Kraków 1838, Pieśni postne, pieśń XVII, s. 116–117 (skan 122–123), <https://pl.wikisource.org/wiki/Śpiewnik_kościelny/Ach_mój_Jezu_jak_ty_klęczysz> | pisownia uwspółcześniona (zkąd → skąd, żywemi → żywymi, zmarłemi → zmarłymi, zostającemi → zostającymi); refren oznaczony w druku „Przyjdź mój Jezu, i t. d.” zapisany raz jako [C] |
