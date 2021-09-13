void dmb() {
	asm ("dmb sy" : : : "memory");
}
