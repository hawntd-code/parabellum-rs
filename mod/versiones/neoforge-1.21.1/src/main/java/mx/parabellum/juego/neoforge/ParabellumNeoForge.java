package mx.parabellum.juego.neoforge;

import mx.parabellum.juego.Juego;
import mx.parabellum.juego.mc.Base;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.event.lifecycle.FMLClientSetupEvent;
import net.neoforged.neoforge.client.event.ClientTickEvent;
import net.neoforged.neoforge.client.event.RegisterKeyMappingsEvent;
import net.neoforged.neoforge.client.event.RenderGuiEvent;
import net.neoforged.neoforge.common.NeoForge;

/** NeoForge 1.21 y 1.21.1: la tecla, el tick y los avisos van con eventos de NeoForge. */
@Mod(value = "parabellum", dist = Dist.CLIENT)
public final class ParabellumNeoForge extends Base {
    public ParabellumNeoForge(IEventBus bus) {
        bus.addListener(RegisterKeyMappingsEvent.class, evento -> evento.register(ABRIR));
        bus.addListener(FMLClientSetupEvent.class, evento -> Juego.iniciar(this));
        NeoForge.EVENT_BUS.addListener(ClientTickEvent.Post.class, evento -> tick());
        NeoForge.EVENT_BUS.addListener(RenderGuiEvent.Post.class, evento -> dibujarAvisos(evento.getGuiGraphics()));
    }

    @Override
    protected String loader() {
        return "NeoForge";
    }
}
