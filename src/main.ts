import { mount } from "svelte";
import App from "./App.svelte";
// Fonts ship with the app so the studio PC needs no internet connection.
import "@fontsource/ibm-plex-sans-condensed/latin-400.css";
import "@fontsource/ibm-plex-sans-condensed/latin-500.css";
import "@fontsource/ibm-plex-sans-condensed/latin-600.css";
import "@fontsource/ibm-plex-sans-condensed/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./app.css";
import { uiScale } from "./lib/uiScale.svelte";

// Zoom out on screens smaller than the 2560x1440 layout, before the first paint where possible.
uiScale.start();

export default mount(App, { target: document.getElementById("app")! });
