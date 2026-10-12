package mx.parabellum.juego.forge;

import mx.parabellum.juego.Juego;
import mx.parabellum.juego.mc.Base;
import net.minecraftforge.client.event.RegisterKeyMappingsEvent;
import net.minecraftforge.client.event.RenderGuiOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.eventbus.api.IEventBus;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.event.lifecycle.FMLClientSetupEvent;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.fml.loading.FMLEnvironment;
import net.minecraftforge.client.event.ScreenEvent;
import net.minecraft.client.gui.screens.PauseScreen;

/** Forge 1.19 a 1.19.2: la tecla, el tick y los avisos van con eventos. */
@Mod("parabellum")
public final class ParabellumForge extends Base {
    public ParabellumForge() {
        if (!FMLEnvironment.dist.isClient()) return;
        IEventBus bus = FMLJavaModLoadingContext.get().getModEventBus();
        bus.addListener((RegisterKeyMappingsEvent evento) -> evento.register(ABRIR));
        bus.addListener((FMLClientSetupEvent evento) -> Juego.iniciar(this));
        MinecraftForge.EVENT_BUS.addListener((TickEvent.ClientTickEvent evento) -> {
            if (evento.phase == TickEvent.Phase.END) tick();
        });
        MinecraftForge.EVENT_BUS.addListener((RenderGuiOverlayEvent.Post evento) -> {
            // Una vez por cuadro: después de la última capa (la del chat).
            if (evento.getOverlay().id().getPath().equals("chat_panel")) dibujarAvisos(evento.getPoseStack());
        });
        MinecraftForge.EVENT_BUS.addListener((ScreenEvent.Init.Post evento) -> {
            if (evento.getScreen() instanceof PauseScreen) evento.addListener(botonPausa(evento.getScreen()));
        });
    }

    @Override
    protected String loader() {
        return "Forge";
    }
}
