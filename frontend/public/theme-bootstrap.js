(function () {
  try {
    var stored = JSON.parse(localStorage.getItem("theme-storage") || "{}");
    var theme = stored.state && stored.state.theme;
    if (
      theme === "dark" ||
      (!theme && window.matchMedia("(prefers-color-scheme: dark)").matches)
    ) {
      document.documentElement.classList.add("dark");
    }
  } catch (e) {}
})();
