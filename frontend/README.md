# EMS Frontend Configuration Platform

## Overview

The frontend is a Vue.js web application used to manage configuration files for the EMS services. It provides an intuitive interface for viewing and editing settings and embeds Grafana dashboards for visualization.

## Technology Stack

- **Framework**: Vue 3
- **UI Library**: Element Plus
- **State Management**: Vuex
- **Router**: Vue Router
- **HTTP Client**: Axios
- **Build Tool**: Vue CLI

## Directory Layout
```
frontend/
├── public/                 # Static assets
├── src/
│   ├── api/                # API wrappers
│   ├── components/         # Common components
│   ├── router/             # Routing
│   ├── store/              # Vuex store
│   ├── utils/              # Helper functions
│   ├── views/              # Pages
│   ├── App.vue             # Root component
│   └── main.js             # Entry point
├── package.json            # Project dependencies and scripts
└── vue.config.js           # Vue CLI config
```

## Key Files

- **package.json** – dependencies and npm scripts
- **.eslintrc.js** – ESLint configuration
- **babel.config.js** – Babel settings
- **vue.config.js** – dev server and build options

## Pages

- **Home.vue** – system overview and entry points to configuration
- **Dashboard.vue** – embeds Grafana panels
- **Config pages** – ModsrvConfig.vue, NetsrvConfig.vue, ComsrvConfig.vue, HissrvConfig.vue, MosquittoConfig.vue

## State Management

Vuex stores service configurations, loading flags and errors. Actions fetch and save data. In development a mock configuration object is used when `useBackend` is false.

## Running the Project
```bash
npm install
npm run serve   # start dev server
npm run build   # build for production
npm run lint    # code linting
```

## Deployment

The project can be containerized with Docker. Nginx serves the built files and proxies API requests.

## Mosquitto Notes

Mosquitto acts as an MQTT broker for device and service messaging. It can be configured via the frontend for ports, authentication and persistence.

## Extending

To add a new configuration page:
1. Create a component under `src/views/config/`
2. Add a route in `src/router/index.js`
3. Include mock data in `src/store/index.js`
4. Add a menu entry in `src/App.vue`

## Tips

- Default dev port is 8080
- API requests proxy to `http://localhost:3001`
- Grafana is proxied to `http://localhost:3000` during development
