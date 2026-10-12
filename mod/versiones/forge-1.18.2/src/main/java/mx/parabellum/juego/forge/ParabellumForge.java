package mx.parabellum.juego.forge;

import mx.parabellum.juego.Juego;
import mx.parabellum.juego.mc.Base;
import net.minecraftforge.client.ClientRegistry;
import net.minecraftforge.client.event.RenderGameOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.eventbus.api.IEventBus;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.event.lifecycle.FMLClientSetupEvent;
import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;
import net.minecraftforge.fml.loading.FMLEnvironment;
import net.minecraftforge.client.event.ScreenEvent;
import net.minecraft.client.gui.screens.PauseScreen;

/** Forge 1.18.2: la tecla, el tick y los avisos van con eventos. */
@Mod("parabellum")
public final class ParabellumForge extends Base {
    public ParabellumForge() {
        if (!FMLEnvironment.dist.isClient()) return;
        IEventBus bus = FMLJavaModLoadingContext.get().getModEventBus();
        bus.addListener((FMLClientSetupEvent evento) -> {
            ClientRegistry.registerKeyBinding(ABRIR);
            Juego.iniciar(this);
        });
        MinecraftForge.EVENT_BUS.addListener((TickEvent.ClientTickEvent evento) -> {
            if (evento.phase == TickEvent.Phase.END) tick();
        });
        MinecraftForge.EVENT_BUS.addListener((RenderGameOverlayEvent.Post evento) -> {
            if (evento.getType() == RenderGameOverlayEvent.ElementType.ALL) dibujarAvisos(evento.getMatrixStack());
        });
        MinecraftForge.EVENT_BUS.addListener((ScreenEvent.InitScreenEvent.Post evento) -> {
            if (evento.getScreen() instanceof PauseScreen) evento.addListener(botonPausa(evento.getScreen()));
        });
    }

    @Override
    protected String loader() {
        return "Forge";
    }
}
