<template>
  <div class="min-h-screen flex items-center justify-center">
    <div class="max-w-4xl mx-auto p-6 space-y-6">
      <div class="bg-gray-800 rounded-lg shadow-md p-6">
        <h2 class="text-2xl font-bold pb-5 text-white">Create User</h2>
        <form
          @submit.prevent="createUser"
          class="flex flex-col md:flex-row gap-4"
        >
          <input
            v-model="newUser.username"
            type="text"
            placeholder="Username"
            required
            class="px-4 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
          />
          <input
            v-model="newUser.password"
            type="password"
            placeholder="Password"
            required
            class="px-4 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
          />
          <select
            v-model="selectedScope"
            class="w-full max-w-xs select select-bordered bg-gray-700 text-white h-12"
          >
            <option value="admin">Admin</option>
            <option value="editor">Editor</option>
            <option value="viewer" selected>Viewer</option>
          </select>
          <AluButton type="submit" size="medium"> Create User </AluButton>
        </form>
      </div>
      <br />
      <div class="bg-gray-800 rounded-lg shadow-md p-6">
        <h2 class="text-2xl font-bold mb-6 text-white">Users</h2>
        <ul class="divide-y divide-gray-600">
          <li
            v-for="user in users"
            :key="user.username"
            class="py-4 px-2 flex justify-between items-center"
          >
            <div class="flex items-center gap-2">
              <span class="text-white font-medium">{{ user.username }}</span>
              <div class="flex gap-1">
                <span
                  v-for="scope in user.scopes"
                  :key="scope"
                  class="badge"
                  :class="{
                    'badge-primary': scope === 'admin',
                    'badge-secondary': scope === 'editor',
                    'badge-accent': scope === 'viewer',
                  }"
                >
                  {{ scope }}
                </span>
              </div>
            </div>
            <AluButton
              @click="deleteUser(user.username)"
              size="small"
              level="danger"
            >
              Delete
            </AluButton>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from "vue";
import AluButton from "@/components/AluButton.vue";

const users = ref([]);
const newUser = ref({
  username: "",
  password: "",
  scopes: ["viewer"], // Default to viewer permission as an array
});
const selectedScope = ref("viewer"); // Default to viewer permission

const loadUsers = async () => {
  try {
    const response = await fetch("/api/users");
    users.value = await response.json();
  } catch (error) {
    console.error("Error loading users:", error);
  }
};

const createUser = async () => {
  try {
    // Add selected scope to the user object as an array
    newUser.value.scopes = [selectedScope.value];

    const response = await fetch("/api/users", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(newUser.value),
    });

    if (!response.ok) throw new Error("Failed to create user");

    alert("User created successfully");
    newUser.value = { username: "", password: "", scopes: ["viewer"] };
    selectedScope.value = "viewer"; // Reset to default
    await loadUsers();
  } catch (error) {
    alert("Error creating user: " + error);
  }
};

const deleteUser = async (username) => {
  if (!confirm(`Are you sure you want to delete user ${username}?`)) return;

  try {
    const response = await fetch(`/api/users/${username}`, {
      method: "DELETE",
    });

    if (!response.ok) throw new Error("Failed to delete user");
    await loadUsers();
  } catch (error) {
    alert("Error deleting user: " + error);
  }
};

onMounted(() => {
  loadUsers();
});
</script>
