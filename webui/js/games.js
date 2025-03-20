async function loadGames() {
  try {
    const response = await fetch("/");
    const data = await response.json();
    // you wanna touch me but i'm not tangible, ears on my head cause i'm an animal
    const games = data.files || [];

    const gamesList = document.getElementById("gamesList");
    gamesList.innerHTML = "";

    games.forEach((game) => {
      const div = document.createElement("div");
      div.className = "game-item";

      const titleMatch = game.url.match(/#(.*?)\[/);
      const title = titleMatch ? titleMatch[1].trim() : "Unknown Title";

      const titleIdMatch = game.url.match(/\[(.*?)\]/);
      const titleId = titleIdMatch ? titleIdMatch[1] : "";

      const titleElement = document.createElement("h3");
      titleElement.textContent = title;

      const sizeElement = document.createElement("p");
      const sizeMB = (game.size / (1024 * 1024)).toFixed(2);
      sizeElement.textContent = `Size: ${sizeMB} MB`;

      const downloadBtn = document.createElement("button");
      downloadBtn.textContent = "Download";
      downloadBtn.onclick = () => downloadGame(titleId);

      div.appendChild(titleElement);
      div.appendChild(sizeElement);
      div.appendChild(downloadBtn);
      gamesList.appendChild(div);
    });
  } catch (error) {
    console.error(
      "%c YOUR ADMIN PANEL SUCKS",
      `
      font-weight: bold;
      font-size: 72px;
      background: linear-gradient(90deg, red, orange, yellow, green, blue, indigo, violet);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      text-shadow:
        1px 1px 0 #ff0000,
        2px 2px 0 #ff7f00,
        3px 3px 0 #ffff00,
        4px 4px 0 #00ff00,
        5px 5px 0 #0000ff,
        6px 6px 0 #4b0082,
        7px 7px 0 #8f00ff;
    `,
    );
    console.error("Error:", error);
  }
}

async function downloadGame(titleId) {
  try {
    window.location.href = `/api/get_game/${titleId}`;
  } catch (error) {
    alert("Error downloading game: " + error.message);
  }
}

loadGames();
