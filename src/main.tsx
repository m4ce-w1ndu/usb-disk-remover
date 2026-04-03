import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import About from "./About";
import "./styles.css";

const isAbout = window.location.pathname === "/about";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isAbout ? <About /> : <App />}
  </React.StrictMode>
);
