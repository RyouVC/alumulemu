document
  .getElementById("createUserForm")
  .addEventListener("submit", async (e) => {
    e.preventDefault();
    const username = document.getElementById("username").value;
    const password = document.getElementById("password").value;

    try {
      const response = await fetch("/api/users", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ username, password }),
      });

      if (!response.ok) throw new Error("Failed to create user");

      alert("User created successfully");
      loadUsers(); // Refresh the users list
    } catch (error) {
      alert("Error creating user: " + error.message);
    }
  });

async function loadUsers() {
  try {
    const response = await fetch("/api/users");
    const users = await response.json();

    const usersList = document.getElementById("usersList");
    usersList.innerHTML = "";

    users.forEach((user) => {
      const li = document.createElement("li");
      li.textContent = user.username;

      const deleteButton = document.createElement("button");
      deleteButton.textContent = "Delete";
      deleteButton.onclick = () => deleteUser(user.username);

      li.appendChild(deleteButton);
      usersList.appendChild(li);
    });
  } catch (error) {
    console.error("Error loading users:", error);
  }
}

async function deleteUser(username) {
  if (!confirm(`Are you sure you want to delete user ${username}?`)) return;

  try {
    const response = await fetch(`/api/users/${username}`, {
      method: "DELETE",
    });

    if (!response.ok) throw new Error("Failed to delete user");

    loadUsers(); // Refresh the list
  } catch (error) {
    alert("Error deleting user: " + error.message);
  }
}

// Load users when page loads
loadUsers();
