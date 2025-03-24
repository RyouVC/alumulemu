<template>
    <div>Title ID: {{ titleId }}</div>
</template>

<script>
export default {
    data() {
        return {
            titleId: null,
        };
    },
    created() {
        this.titleId = this.$route.query.tid;
    },
};
</script>
