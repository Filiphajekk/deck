// Tests only cover plain TS logic, so no Vite plugins (React/Tailwind) are needed here.
export default {
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
};
