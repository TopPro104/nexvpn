const RI_BASE = 0x1f1e6;

const COUNTRY_NAMES: Record<string, string> = {
  germany: "de", deutschland: "de",
  netherlands: "nl", holland: "nl",
  "united states": "us", usa: "us", america: "us", "los angeles": "us", dallas: "us", "new york": "us", chicago: "us", miami: "us", seattle: "us", "san jose": "us",
  "united kingdom": "gb", uk: "gb", england: "gb", britain: "gb", london: "gb",
  france: "fr", paris: "fr",
  japan: "jp", tokyo: "jp", osaka: "jp",
  singapore: "sg",
  russia: "ru", "\u0440\u043e\u0441\u0441\u0438\u044f": "ru", moscow: "ru",
  turkey: "tr", istanbul: "tr", "\u0442\u0443\u0440\u0446\u0438\u044f": "tr",
  canada: "ca", toronto: "ca", montreal: "ca", vancouver: "ca",
  australia: "au", sydney: "au", melbourne: "au",
  "hong kong": "hk",
  "south korea": "kr", korea: "kr", seoul: "kr",
  india: "in", mumbai: "in",
  brazil: "br", "\u0073\u00e3o paulo": "br",
  italy: "it", milan: "it", rome: "it",
  spain: "es", madrid: "es",
  sweden: "se", stockholm: "se",
  switzerland: "ch", zurich: "ch",
  finland: "fi", helsinki: "fi",
  norway: "no", oslo: "no",
  poland: "pl", warsaw: "pl",
  ukraine: "ua", "\u0443\u043a\u0440\u0430\u0457\u043d\u0430": "ua", kyiv: "ua",
  israel: "il",
  taiwan: "tw",
  ireland: "ie", dublin: "ie",
  austria: "at", vienna: "at",
  belgium: "be", brussels: "be",
  czech: "cz", czechia: "cz", prague: "cz",
  denmark: "dk", copenhagen: "dk",
  romania: "ro", bucharest: "ro",
  bulgaria: "bg", sofia: "bg",
  hungary: "hu", budapest: "hu",
  portugal: "pt", lisbon: "pt",
  argentina: "ar", "buenos aires": "ar",
  mexico: "mx",
  chile: "cl",
  colombia: "co",
  indonesia: "id", jakarta: "id",
  malaysia: "my",
  thailand: "th", bangkok: "th",
  vietnam: "vn",
  philippines: "ph",
  "new zealand": "nz", auckland: "nz",
  "south africa": "za", johannesburg: "za",
  egypt: "eg", cairo: "eg",
  uae: "ae", emirates: "ae", dubai: "ae",
  kazakhstan: "kz", "\u043a\u0430\u0437\u0430\u0445\u0441\u0442\u0430\u043d": "kz",
  latvia: "lv", riga: "lv",
  lithuania: "lt", vilnius: "lt",
  estonia: "ee", tallinn: "ee",
  georgia: "ge", tbilisi: "ge",
  serbia: "rs", belgrade: "rs",
  croatia: "hr", zagreb: "hr",
  iceland: "is", reykjavik: "is",
  luxembourg: "lu",
  moldova: "md",
  albania: "al",
  cyprus: "cy",
  greece: "gr", athens: "gr",
  china: "cn", beijing: "cn", shanghai: "cn",
  iran: "ir", tehran: "ir",
  pakistan: "pk",
  bangladesh: "bd",
  nigeria: "ng", lagos: "ng",
  kenya: "ke", nairobi: "ke",
  amsterdam: "nl", frankfurt: "de", berlin: "de", munich: "de",
};

const KNOWN_CODES = new Set([
  "ad","ae","af","ag","al","am","ao","ar","at","au","az","ba","bb","bd","be","bf","bg",
  "bh","bi","bj","bn","bo","br","bs","bt","bw","by","bz","ca","cd","cf","cg","ch","ci",
  "cl","cm","cn","co","cr","cu","cv","cy","cz","de","dj","dk","dm","do","dz","ec","ee",
  "eg","er","es","et","fi","fj","fr","ga","gb","gd","ge","gh","gm","gn","gq","gr","gt",
  "gw","gy","hk","hn","hr","ht","hu","id","ie","il","in","iq","ir","is","it","jm","jo",
  "jp","ke","kg","kh","km","kn","kp","kr","kw","kz","la","lb","lc","li","lk","lr","ls",
  "lt","lu","lv","ly","ma","mc","md","me","mg","mk","ml","mm","mn","mo","mr","mt","mu",
  "mv","mw","mx","my","mz","na","ne","ng","ni","nl","no","np","nz","om","pa","pe","pg",
  "ph","pk","pl","pt","py","qa","ro","rs","ru","rw","sa","sb","sc","sd","se","sg","si",
  "sk","sl","sm","sn","so","sr","ss","st","sv","sy","sz","td","tg","th","tj","tl","tm",
  "tn","to","tr","tt","tv","tw","tz","ua","ug","us","uy","uz","va","vc","ve","vn","vu",
  "ws","xk","ye","za","zm","zw",
]);

export function extractCountryCode(name: string): string | null {
  // 1. Flag emoji regional indicators
  for (let i = 0; i < name.length; i++) {
    const cp = name.codePointAt(i);
    if (cp && cp >= RI_BASE && cp <= RI_BASE + 25) {
      const nextIdx = cp > 0xffff ? i + 2 : i + 1;
      const next = name.codePointAt(nextIdx);
      if (next && next >= RI_BASE && next <= RI_BASE + 25) {
        const a = String.fromCharCode(cp - RI_BASE + 65);
        const b = String.fromCharCode(next - RI_BASE + 65);
        return (a + b).toLowerCase();
      }
    }
  }

  // 2. [XX] pattern
  const bracket = name.match(/\[([A-Za-z]{2})\]/);
  if (bracket && KNOWN_CODES.has(bracket[1].toLowerCase())) {
    return bracket[1].toLowerCase();
  }

  // 3. XX- or XX | or XX : prefix
  const prefix = name.match(/^([A-Za-z]{2})\s*[-|:]/);
  if (prefix && KNOWN_CODES.has(prefix[1].toLowerCase())) {
    return prefix[1].toLowerCase();
  }

  // 4. Country / city name anywhere in string
  const lower = name.toLowerCase();
  for (const [keyword, code] of Object.entries(COUNTRY_NAMES)) {
    if (lower.includes(keyword)) return code;
  }

  return null;
}
