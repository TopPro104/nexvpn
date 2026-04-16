export type Lang = "en" | "ru";

const translations = {
  // Navigation
  "nav.home": { en: "Home", ru: "Главная" },
  "nav.subscriptions": { en: "Subscriptions", ru: "Подписки" },
  "nav.stats": { en: "Statistics", ru: "Статистика" },
  "nav.logs": { en: "Logs", ru: "Логи" },
  "nav.routing": { en: "Routing", ru: "Маршруты" },
  "nav.privacy": { en: "Privacy", ru: "Приватность" },
  "nav.speedtest": { en: "Speed Test", ru: "Тест скорости" },
  "nav.settings": { en: "Settings", ru: "Настройки" },

  // Status
  "status.connected": { en: "Connected", ru: "Подключено" },
  "status.disconnected": { en: "Disconnected", ru: "Отключено" },
  "status.connecting": { en: "Connecting...", ru: "Подключение..." },

  // Server list
  "servers.search": { en: "Search servers...", ru: "Поиск серверов..." },
  "servers.import": { en: "+ Import", ru: "+ Импорт" },
  "servers.pingAll": { en: "Ping All", ru: "Пинг всех" },
  "servers.autoSelect": { en: "Auto-select", ru: "Автовыбор" },
  "servers.all": { en: "All", ru: "Все" },
  "servers.manual": { en: "Manual", ru: "Вручную" },
  "servers.empty": {
    en: "No servers yet. Import links or add a subscription.",
    ru: "Серверов пока нет. Добавьте ссылки или подписку.",
  },
  "servers.noMatch": {
    en: "No servers match your filter.",
    ru: "Ничего не найдено по фильтру.",
  },
  "servers.importPlaceholder": {
    en: "Paste links (vless://, vmess://, ss://, trojan://) or subscription URL",
    ru: "Вставьте ссылки (vless://, vmess://, ss://, trojan://) или URL подписки",
  },
  "servers.cancel": { en: "Cancel", ru: "Отмена" },
  "servers.selectFirst": { en: "Select a server first", ru: "Сначала выберите сервер" },
  "servers.bestServer": { en: "Best server", ru: "Лучший сервер" },
  "servers.remove": { en: "Remove", ru: "Удалить" },
  "servers.sortName": { en: "By name", ru: "По имени" },
  "servers.sortPing": { en: "By ping", ru: "По пингу" },
  "servers.sortProto": { en: "By protocol", ru: "По протоколу" },

  // Subscriptions
  "subs.title": { en: "Subscriptions", ru: "Подписки" },
  "subs.add": { en: "+ Add Subscription", ru: "+ Добавить подписку" },
  "subs.empty": {
    en: "No subscriptions yet. Add one to get started.",
    ru: "Подписок пока нет. Добавьте первую, чтобы начать.",
  },
  "subs.servers": { en: "servers", ru: "серверов" },
  "subs.updated": { en: "Updated", ru: "Обновлено" },
  "subs.never": { en: "Never", ru: "Никогда" },
  "subs.update": { en: "Update", ru: "Обновить" },
  "subs.delete": { en: "Delete", ru: "Удалить" },
  "subs.addTitle": { en: "Add Subscription", ru: "Добавить подписку" },
  "subs.urlLabel": { en: "Subscription URL", ru: "URL подписки" },
  "subs.nameLabel": { en: "Name (optional)", ru: "Название (необязательно)" },
  "subs.urlPlaceholder": { en: "https://example.com/sub", ru: "https://example.com/sub" },
  "subs.namePlaceholder": { en: "My subscription", ru: "Моя подписка" },
  "subs.reliableSource": { en: "Reliable VPN source", ru: "Надёжный источник VPN" },
  "subs.traffic": { en: "Traffic", ru: "Трафик" },
  "subs.unlimited": { en: "Unlimited", ru: "Безлимит" },
  "subs.expires": { en: "Expires", ru: "Истекает" },
  "subs.expired": { en: "Expired", ru: "Истекла" },
  "subs.refill": { en: "Traffic reset", ru: "Сброс трафика" },
  "subs.support": { en: "Support", ru: "Поддержка" },
  "subs.manage": { en: "Manage", ru: "Управление" },
  "subs.autoUpdate": { en: "Auto-update", ru: "Автообновление" },
  "subs.hours": { en: "h", ru: "ч" },
  "subs.announce": { en: "Announcement", ru: "Объявление" },
  "toast.subAutoUpdated": { en: "Subscription auto-updated", ru: "Подписка автообновлена" },
  "toast.subDuplicate": { en: "This subscription URL already exists", ru: "Эта подписка уже добавлена" },
  "subs.confirmDelete": { en: "Delete subscription", ru: "Удалить подписку" },
  "subs.willBeRemoved": { en: "will be removed", ru: "будут удалены" },
  "servers.confirmDelete": { en: "Delete server", ru: "Удалить сервер" },

  // Settings
  "settings.title": { en: "Settings", ru: "Настройки" },
  "settings.theme": { en: "Theme", ru: "Тема" },
  "settings.style": { en: "Style", ru: "Стиль" },
  "settings.core": { en: "Core Engine", ru: "Ядро" },
  "settings.vpnMode": { en: "VPN Mode", ru: "Режим VPN" },
  "settings.vpnMode.proxy": { en: "System Proxy", ru: "Системный прокси" },
  "settings.vpnMode.proxyDesc": { en: "Sets HTTP proxy in system settings. Works for most browsers and apps.", ru: "Прописывает HTTP-прокси в системе. Работает в большинстве браузеров и приложений." },
  "settings.vpnMode.tun": { en: "TUN (Full VPN)", ru: "TUN (полный VPN)" },
  "settings.vpnMode.tunDesc": { en: "Creates a virtual network interface that captures ALL system traffic. Requires admin rights and wintun.dll.", ru: "Виртуальный сетевой интерфейс, который перехватывает весь трафик. Нужны права администратора и wintun.dll." },
  "settings.vpnMode.requestAdmin": { en: "Run as Administrator", ru: "Запустить от имени администратора" },
  "settings.ports": { en: "Ports", ru: "Порты" },
  "settings.ports.auto": { en: "Automatic", ru: "Автоматически" },
  "settings.ports.manual": { en: "Manual", ru: "Вручную" },
  "settings.ports.autoDesc": { en: "Ports are randomized on each connection for better security", ru: "Порты выбираются случайно при каждом подключении для безопасности" },
  "perApp.mode": { en: "Per-App VPN", ru: "VPN по приложениям" },
  "perApp.all": { en: "All apps", ru: "Все приложения" },
  "perApp.include": { en: "Only selected", ru: "Только выбранные" },
  "perApp.exclude": { en: "All except selected", ru: "Все кроме выбранных" },
  "perApp.includeDesc": { en: "Only selected apps will use VPN", ru: "Только выбранные приложения будут использовать VPN" },
  "perApp.excludeDesc": { en: "Selected apps will bypass VPN", ru: "Выбранные приложения будут обходить VPN" },
  "perApp.search": { en: "Search apps...", ru: "Поиск приложений..." },
  "perApp.selected": { en: "selected", ru: "выбрано" },
  "perApp.noApps": { en: "No apps found", ru: "Приложения не найдены" },
  "perApp.loading": { en: "Loading apps...", ru: "Загрузка приложений..." },
  "perApp.tunOnly": { en: "Per-app VPN only works in TUN mode. Switch to TUN in VPN settings.", ru: "VPN по приложениям работает только в TUN-режиме. Переключите на TUN в настройках VPN." },
  "settings.stealth": { en: "Stealth Mode (Root)", ru: "Режим невидимости (Root)" },
  "settings.stealthDesc": { en: "Experimental: uses root to hide VPN from detection apps (fixes MTU, blocks port scanning)", ru: "Экспериментально: использует root для скрытия VPN от приложений-детекторов (исправляет MTU, блокирует сканирование портов)" },
  "settings.xposed": { en: "Full VPN Hiding (LSPosed)", ru: "Полное скрытие VPN (LSPosed)" },
  "settings.xposedBuiltIn": { en: "Xposed module is built into NexVPN", ru: "Xposed-модуль встроен в NexVPN" },
  "settings.xposedActive": { en: "Module active — VPN hidden", ru: "Модуль активен — VPN скрыт" },
  "settings.xposedPending": { en: "LSPosed detected — enable module", ru: "LSPosed обнаружен — включите модуль" },
  "settings.xposedPendingDesc": { en: "LSPosed is installed. Enable NexVPN module and select target apps.", ru: "LSPosed установлен. Включите модуль NexVPN и выберите целевые приложения." },
  "settings.xposedInactive": { en: "Module not active", ru: "Модуль не активен" },
  "settings.xposedHooked": { en: "Protected apps", ru: "Защищённые приложения" },
  "settings.xposedDesc": { en: "Completely hides VPN from any app: removes TRANSPORT_VPN flag, hides tun interface, removes VPN apps from package list. Requires root + LSPosed.", ru: "Полностью скрывает VPN от любого приложения: убирает флаг TRANSPORT_VPN, скрывает tun-интерфейс, убирает VPN-приложения из списка пакетов. Требуется root + LSPosed." },
  "settings.xposedStep1": { en: "Install Magisk + LSPosed", ru: "Установите Magisk + LSPosed" },
  "settings.xposedStep2": { en: "Open LSPosed → Modules → Enable NexVPN", ru: "Откройте LSPosed → Модули → Включите NexVPN" },
  "settings.xposedStep3": { en: "In scope select apps to hide VPN from (banks, checkers)", ru: "В scope выберите приложения от которых скрыть VPN (банки, чекеры)" },
  "settings.xposedStep4": { en: "Reboot device", ru: "Перезагрузите устройство" },
  "settings.autoConnect": { en: "Auto-connect on startup", ru: "Подключаться при запуске" },
  "settings.autoReconnect": { en: "Auto-reconnect on drop", ru: "Переподключаться при разрыве" },
  "settings.language": { en: "Language", ru: "Язык" },
  "settings.importExport": { en: "Import / Export", ru: "Импорт / Экспорт" },
  "settings.export": { en: "Export Config", ru: "Экспорт конфига" },
  "settings.importBtn": { en: "Import Config", ru: "Импорт конфига" },
  "settings.hwid": { en: "Send device info", ru: "Отправлять информацию об устройстве" },
  "settings.hwidDesc": { en: "Sends HWID and device info with subscription requests. Required by some panels.", ru: "Передаёт HWID и информацию об устройстве вместе с запросами подписок. Нужно для некоторых панелей." },
  "settings.hwidCopy": { en: "Copy", ru: "Копировать" },
  "settings.hwidCopied": { en: "Copied!", ru: "Скопировано!" },
  "settings.hwidPlatform": { en: "Platform", ru: "Платформа" },
  "settings.hwidOsVersion": { en: "OS Version", ru: "Версия ОС" },
  "settings.hwidModel": { en: "Model", ru: "Модель" },

  // Logs
  "logs.filter": { en: "Filter logs...", ru: "Фильтр логов..." },
  "logs.autoScroll": { en: "Auto-scroll", ru: "Автоскролл" },
  "logs.copy": { en: "Copy", ru: "Копировать" },
  "logs.clear": { en: "Clear", ru: "Очистить" },
  "logs.core": { en: "Core", ru: "Ядро" },
  "logs.app": { en: "App", ru: "Приложение" },
  "logs.empty": { en: "No logs yet. Connect to a server to see core output.", ru: "Логов пока нет. Подключитесь к серверу, чтобы увидеть вывод ядра." },
  "logs.appEmpty": { en: "No app logs yet. Update a subscription to see parsing results.", ru: "Логов приложения пока нет. Обновите подписку, чтобы увидеть результат парсинга." },

  // Animation
  "settings.animation": { en: "Animation", ru: "Анимация" },
  "settings.animation.none": { en: "None", ru: "Нет" },
  "settings.animation.smooth": { en: "Smooth", ru: "Плавная" },
  "settings.animation.energetic": { en: "Energetic", ru: "Энергичная" },

  // Stats
  "stats.reset": { en: "Reset", ru: "Сбросить" },
  "stats.resetConfirm": { en: "Reset all connection statistics?", ru: "Сбросить всю статистику подключений?" },
  "toast.statsReset": { en: "Statistics reset", ru: "Статистика сброшена" },
  "stats.speedGraph": { en: "Download Speed", ru: "Скорость загрузки" },
  "stats.connectToSee": { en: "Connect to see speed graph", ru: "Подключитесь, чтобы увидеть график скорости" },
  "stats.sessions": { en: "Sessions", ru: "Сессий" },
  "stats.totalTime": { en: "Total Time", ru: "Общее время" },
  "stats.totalUpload": { en: "Total Upload", ru: "Всего отправлено" },
  "stats.totalDownload": { en: "Total Download", ru: "Всего загружено" },
  "stats.history": { en: "Connection History", ru: "История подключений" },
  "stats.noHistory": { en: "No connection history yet.", ru: "Истории подключений пока нет." },
  "stats.server": { en: "Server", ru: "Сервер" },
  "stats.duration": { en: "Duration", ru: "Длительность" },
  "stats.traffic": { en: "Traffic", ru: "Трафик" },
  "stats.date": { en: "Date", ru: "Дата" },

  // Routing
  "routing.title": { en: "Routing Rules", ru: "Правила маршрутизации" },
  "routing.defaultRoute": { en: "Default Route", ru: "Маршрут по умолчанию" },
  "routing.proxyAll": { en: "Proxy All", ru: "Всё через прокси" },
  "routing.proxyAllDesc": { en: "All traffic goes through VPN. Add direct rules to bypass specific domains.", ru: "Весь трафик идёт через VPN. Добавьте правила «напрямую», чтобы обойти нужные домены." },
  "routing.directAll": { en: "Direct All (Split Tunnel)", ru: "Всё напрямую (раздельное туннелирование)" },
  "routing.directAllDesc": { en: "Only proxy-rule domains go through VPN. Everything else is direct.", ru: "Через VPN идут только домены с правилом «прокси». Всё остальное — напрямую." },
  "routing.rules": { en: "Rules", ru: "Правила" },
  "routing.addRule": { en: "Add Rule", ru: "Добавить правило" },
  "routing.domain": { en: "Domain", ru: "Домен" },
  "routing.domainPlaceholder": { en: "e.g. google.com", ru: "напр. google.com" },
  "routing.action": { en: "Action", ru: "Действие" },
  "routing.proxy": { en: "Proxy", ru: "Прокси" },
  "routing.direct": { en: "Direct", ru: "Напрямую" },
  "routing.block": { en: "Block", ru: "Блокировать" },
  "routing.noRules": { en: "No routing rules yet. Add one above.", ru: "Правил пока нет. Добавьте первое выше." },
  "routing.presets": { en: "Quick Presets", ru: "Быстрые шаблоны" },
  "routing.presetAds": { en: "Block Ads", ru: "Блокировка рекламы" },
  "routing.presetRuDirect": { en: "RU sites direct", ru: "RU сайты напрямую" },
  "routing.saved": { en: "Routing rules saved", ru: "Правила маршрутизации сохранены" },

  // Onboarding
  "onboarding.welcome": { en: "Welcome to NexVPN!", ru: "Добро пожаловать в NexVPN!" },
  "onboarding.skip": { en: "Skip", ru: "Пропустить" },
  "onboarding.next": { en: "Next", ru: "Дальше" },
  "onboarding.showMe": { en: "Show me around", ru: "Начать обзор" },
  "onboarding.stepHome": { en: "Connect to VPN here. Choose a server from the list below.", ru: "Здесь вы подключаетесь к VPN. Выберите сервер из списка ниже и нажмите кнопку." },
  "onboarding.stepSubs": { en: "Import subscription URLs or paste server links.", ru: "Сюда можно добавить подписки на серверы или вставить ссылки вручную." },
  "onboarding.stepRouting": { en: "Set routing rules, block ads, enable split tunneling.", ru: "Тут настраиваются правила маршрутизации: блокировка рекламы, раздельное туннелирование и прочее." },
  "onboarding.stepSettings": { en: "Customize themes, VPN mode, language and more.", ru: "Здесь можно сменить тему, режим VPN, язык и другие настройки." },
  "onboarding.finish": { en: "You're all set!", ru: "Готово, можно начинать!" },
  "onboarding.blockAdsNow": { en: "Block Ads now", ru: "Включить блокировку рекламы" },
  "onboarding.startBrowsing": { en: "Start browsing", ru: "Перейти к работе" },

  // Common
  "common.add": { en: "Add", ru: "Добавить" },
  "common.cancel": { en: "Cancel", ru: "Отмена" },
  "common.import": { en: "Import", ru: "Импорт" },
  "common.confirm": { en: "Confirm", ru: "Подтверждение" },
  "common.yes": { en: "Yes", ru: "Да" },

  // Toast messages
  "toast.disconnected": { en: "Disconnected", ru: "Отключено" },
  "toast.connectedTo": { en: "Connected to", ru: "Подключено к" },
  "toast.pingDone": { en: "Ping done", ru: "Пинг завершён" },
  "toast.reachable": { en: "reachable", ru: "доступно" },
  "toast.subAdded": { en: "Subscription added", ru: "Подписка добавлена" },
  "toast.imported": { en: "Imported", ru: "Импортировано" },
  "toast.subUpdated": { en: "Updated", ru: "Обновлено" },
  "toast.subDeleted": { en: "Subscription deleted", ru: "Подписка удалена" },
  "toast.configExported": { en: "Config exported", ru: "Конфиг экспортирован" },
  "toast.settingsSaved": { en: "Settings saved", ru: "Настройки сохранены" },
  "toast.portRange": { en: "Port must be 1-65535", ru: "Порт должен быть 1-65535" },
  "toast.portsDifferent": {
    en: "SOCKS and HTTP ports must be different",
    ru: "Порты SOCKS и HTTP должны отличаться",
  },
  "toast.initFailed": { en: "Init failed", ru: "Не удалось инициализировать" },

  // Privacy Shield
  "privacy.title": { en: "Privacy Shield", ru: "Щит приватности" },
  "privacy.score": { en: "Privacy Score", ru: "Оценка приватности" },
  "privacy.runChecks": { en: "Run Checks", ru: "Запустить проверку" },
  "privacy.running": { en: "Checking...", ru: "Проверяем..." },
  "privacy.ipCheck": { en: "IP Address", ru: "IP адрес" },
  "privacy.ipDesc": { en: "Your public IP address visible to websites", ru: "Ваш публичный IP, видимый сайтам" },
  "privacy.dnsCheck": { en: "DNS Leak Test", ru: "Тест утечки DNS" },
  "privacy.dnsDesc": { en: "Check if DNS requests leak outside VPN", ru: "Проверка утечки DNS-запросов мимо VPN" },
  "privacy.encryption": { en: "Encryption", ru: "Шифрование" },
  "privacy.encDesc": { en: "Traffic encryption status", ru: "Статус шифрования трафика" },
  "privacy.killSwitch": { en: "VPN Mode", ru: "Режим VPN" },
  "privacy.killDesc": { en: "Network protection level", ru: "Уровень защиты сети" },
  "privacy.protected": { en: "Protected", ru: "Защищено" },
  "privacy.exposed": { en: "Exposed", ru: "Не защищено" },
  "privacy.encrypted": { en: "Encrypted", ru: "Зашифровано" },
  "privacy.tunActive": { en: "TUN (Full Protection)", ru: "TUN (полная защита)" },
  "privacy.proxyActive": { en: "Proxy Mode", ru: "Режим прокси" },
  "privacy.connectFirst": { en: "Connect to VPN to run privacy checks", ru: "Подключитесь к VPN для проверки приватности" },
  "privacy.noLeak": { en: "No DNS leak detected", ru: "Утечки DNS не обнаружено" },
  "privacy.leakWarning": { en: "Possible DNS leak", ru: "Возможна утечка DNS" },

  // Speed Test
  "speedtest.title": { en: "Speed Test", ru: "Тест скорости" },
  "speedtest.start": { en: "Start Test", ru: "Начать тест" },
  "speedtest.testing": { en: "Testing...", ru: "Тестируем..." },
  "speedtest.download": { en: "Download", ru: "Загрузка" },
  "speedtest.upload": { en: "Upload", ru: "Выгрузка" },
  "speedtest.ping": { en: "Ping", ru: "Пинг" },
  "speedtest.mbps": { en: "Mbps", ru: "Мбит/с" },
  "speedtest.ms": { en: "ms", ru: "мс" },
  "speedtest.history": { en: "Test History", ru: "История тестов" },
  "speedtest.noHistory": { en: "No tests yet. Run a speed test to see results.", ru: "Тестов пока нет. Запустите тест скорости." },
  "speedtest.connectFirst": { en: "Connect to VPN to test speed", ru: "Подключитесь к VPN для теста скорости" },

  // Dashboard (enhanced stats)
  "stats.dashboard": { en: "Dashboard", ru: "Дашборд" },
  "stats.dailyTraffic": { en: "Daily Traffic (7 days)", ru: "Трафик по дням (7 дней)" },
  "stats.topServers": { en: "Top Servers", ru: "Топ серверов" },
  "stats.protocols": { en: "Protocol Distribution", ru: "Протоколы" },
  "stats.protocolMix": { en: "Protocols", ru: "Протоколы" },
  "stats.connections": { en: "connections", ru: "подключений" },

  // Server list extras
  "servers.favorites": { en: "Favorites", ru: "Избранные" },
  "servers.sortCountry": { en: "By country", ru: "По стране" },
  "servers.gridView": { en: "Grid view", ru: "Сетка" },
  "servers.listView": { en: "List view", ru: "Список" },

  // Auto-update
  "update.title": { en: "Updates", ru: "Обновления" },
  "update.check": { en: "Check for Updates", ru: "Проверить обновления" },
  "update.checking": { en: "Checking...", ru: "Проверяем..." },
  "update.available": { en: "Update Available", ru: "Доступно обновление" },
  "update.upToDate": { en: "You're up to date!", ru: "У вас последняя версия!" },
  "update.current": { en: "Current", ru: "Текущая" },
  "update.latest": { en: "Latest", ru: "Последняя" },
  "update.download": { en: "Download Update", ru: "Скачать обновление" },
  "update.changelog": { en: "Changelog", ru: "Изменения" },
  "update.error": { en: "Failed to check for updates", ru: "Не удалось проверить обновления" },
  "update.devHint": { en: "Development build — ahead of public release", ru: "Dev-сборка — впереди публичного релиза" },

  // Quick Connect
  "quick.recent": { en: "Recent", ru: "Недавние" },
  "quick.fastest": { en: "Fastest", ru: "Быстрейший" },

  // Connection Info
  "connInfo.time": { en: "Time", ru: "Время" },
  "connInfo.mode": { en: "Mode", ru: "Режим" },
} as const;

export type TranslationKey = keyof typeof translations;

let currentLang: Lang = "en";

export function setLang(lang: Lang) {
  currentLang = lang;
}

export function getLang(): Lang {
  return currentLang;
}

export function t(key: TranslationKey): string {
  const entry = translations[key];
  if (!entry) return key;
  return entry[currentLang] || entry.en;
}
