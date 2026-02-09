export type Lang = "en" | "ru";

const translations = {
  // Navigation
  "nav.home": { en: "Home", ru: "Главная" },
  "nav.subscriptions": { en: "Subscriptions", ru: "Подписки" },
  "nav.stats": { en: "Statistics", ru: "Статистика" },
  "nav.logs": { en: "Logs", ru: "Логи" },
  "nav.routing": { en: "Routing", ru: "Маршруты" },
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
    ru: "Нет серверов. Импортируйте ссылки или добавьте подписку.",
  },
  "servers.noMatch": {
    en: "No servers match your filter.",
    ru: "Серверы не найдены по фильтру.",
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
    ru: "Нет подписок. Добавьте первую для начала.",
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
  "subs.reliableSource": { en: "Reliable VPN source", ru: "Надёжный VPN источник" },

  // Settings
  "settings.title": { en: "Settings", ru: "Настройки" },
  "settings.theme": { en: "Theme", ru: "Тема" },
  "settings.style": { en: "Style", ru: "Стиль" },
  "settings.core": { en: "Core Engine", ru: "Ядро" },
  "settings.vpnMode": { en: "VPN Mode", ru: "Режим VPN" },
  "settings.vpnMode.proxy": { en: "System Proxy", ru: "Системный прокси" },
  "settings.vpnMode.proxyDesc": { en: "Sets HTTP proxy in system settings. Works for most browsers and apps.", ru: "Устанавливает HTTP-прокси в системных настройках. Работает для большинства браузеров и приложений." },
  "settings.vpnMode.tun": { en: "TUN (Full VPN)", ru: "TUN (Полный VPN)" },
  "settings.vpnMode.tunDesc": { en: "Creates a virtual network interface that captures ALL system traffic. Requires admin rights and wintun.dll.", ru: "Создаёт виртуальный сетевой интерфейс, перехватывающий ВЕСЬ трафик системы. Требует прав администратора и wintun.dll." },
  "settings.ports": { en: "Ports", ru: "Порты" },
  "settings.autoConnect": { en: "Auto-connect on startup", ru: "Автоподключение при запуске" },
  "settings.autoReconnect": { en: "Auto-reconnect on drop", ru: "Автопереподключение при обрыве" },
  "settings.language": { en: "Language", ru: "Язык" },
  "settings.importExport": { en: "Import / Export", ru: "Импорт / Экспорт" },
  "settings.export": { en: "Export Config", ru: "Экспорт конфига" },
  "settings.importBtn": { en: "Import Config", ru: "Импорт конфига" },
  "settings.hwid": { en: "Send device info", ru: "Отправлять информацию об устройстве" },
  "settings.hwidDesc": { en: "Sends HWID and device info with subscription requests. Required by some panels.", ru: "Отправляет HWID и данные устройства при запросах подписок. Требуется некоторыми панелями." },
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
  "logs.empty": { en: "No logs yet. Connect to a server to see core output.", ru: "Пока нет логов. Подключитесь к серверу." },

  // Animation
  "settings.animation": { en: "Animation", ru: "Анимация" },
  "settings.animation.none": { en: "None", ru: "Без" },
  "settings.animation.smooth": { en: "Smooth", ru: "Плавная" },
  "settings.animation.energetic": { en: "Energetic", ru: "Энергичная" },

  // Stats
  "stats.reset": { en: "Reset", ru: "Сбросить" },
  "stats.resetConfirm": { en: "Reset all connection statistics?", ru: "Сбросить всю статистику подключений?" },
  "toast.statsReset": { en: "Statistics reset", ru: "Статистика сброшена" },
  "stats.speedGraph": { en: "Download Speed", ru: "Скорость загрузки" },
  "stats.connectToSee": { en: "Connect to see speed graph", ru: "Подключитесь для графика скорости" },
  "stats.sessions": { en: "Sessions", ru: "Сессий" },
  "stats.totalTime": { en: "Total Time", ru: "Общее время" },
  "stats.totalUpload": { en: "Total Upload", ru: "Всего отправлено" },
  "stats.totalDownload": { en: "Total Download", ru: "Всего загружено" },
  "stats.history": { en: "Connection History", ru: "История подключений" },
  "stats.noHistory": { en: "No connection history yet.", ru: "История подключений пуста." },
  "stats.server": { en: "Server", ru: "Сервер" },
  "stats.duration": { en: "Duration", ru: "Длительность" },
  "stats.traffic": { en: "Traffic", ru: "Трафик" },
  "stats.date": { en: "Date", ru: "Дата" },

  // Routing
  "routing.title": { en: "Routing Rules", ru: "Правила маршрутизации" },
  "routing.defaultRoute": { en: "Default Route", ru: "Маршрут по умолчанию" },
  "routing.proxyAll": { en: "Proxy All", ru: "Всё через прокси" },
  "routing.proxyAllDesc": { en: "All traffic goes through VPN. Add direct rules to bypass specific domains.", ru: "Весь трафик идёт через VPN. Добавьте правила direct для обхода отдельных доменов." },
  "routing.directAll": { en: "Direct All (Split Tunnel)", ru: "Всё напрямую (Split Tunnel)" },
  "routing.directAllDesc": { en: "Only proxy-rule domains go through VPN. Everything else is direct.", ru: "Только домены с правилом proxy идут через VPN. Остальное напрямую." },
  "routing.rules": { en: "Rules", ru: "Правила" },
  "routing.addRule": { en: "Add Rule", ru: "Добавить правило" },
  "routing.domain": { en: "Domain", ru: "Домен" },
  "routing.domainPlaceholder": { en: "e.g. google.com", ru: "напр. google.com" },
  "routing.action": { en: "Action", ru: "Действие" },
  "routing.proxy": { en: "Proxy", ru: "Прокси" },
  "routing.direct": { en: "Direct", ru: "Напрямую" },
  "routing.block": { en: "Block", ru: "Блокировать" },
  "routing.noRules": { en: "No routing rules yet. Add one above.", ru: "Нет правил. Добавьте выше." },
  "routing.presets": { en: "Quick Presets", ru: "Быстрые шаблоны" },
  "routing.presetAds": { en: "Block Ads", ru: "Блокировка рекламы" },
  "routing.presetRuDirect": { en: "RU sites direct", ru: "RU сайты напрямую" },
  "routing.saved": { en: "Routing rules saved", ru: "Правила маршрутизации сохранены" },

  // Common
  "common.add": { en: "Add", ru: "Добавить" },
  "common.cancel": { en: "Cancel", ru: "Отмена" },
  "common.import": { en: "Import", ru: "Импорт" },

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
  "toast.initFailed": { en: "Init failed", ru: "Ошибка инициализации" },
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
