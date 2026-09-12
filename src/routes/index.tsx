import { createFileRoute } from "@tanstack/react-router";
import { PeakstrifeApp } from "@/game/PeakstrifeApp";

export const Route = createFileRoute("/")({ component: Home });

function Home() {
  return <PeakstrifeApp />;
}
