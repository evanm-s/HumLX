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

export default mount(App, { target: document.getElementById("app")! });
